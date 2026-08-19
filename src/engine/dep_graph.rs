use std::collections::{HashMap, HashSet};
use crate::models::InventoryItem;

/// Dependency Graph & Orphan Resolver
pub struct DependencyGraph {
    /// Inverted index: Dependency ID -> Set of Parent IDs that require it
    #[allow(dead_code)]
    reverse_deps: HashMap<String, HashSet<String>>,
    /// Set of known orphan dependency IDs
    orphans: HashSet<String>,
}

impl DependencyGraph {
    /// Builds dependency index across all items and resolves required_by links
    pub fn resolve(items: &mut [InventoryItem]) -> Self {
        let mut reverse_deps: HashMap<String, HashSet<String>> = HashMap::new();

        // 1. Build reverse dependency mappings from declared dependencies
        for item in items.iter() {
            for dep_id in &item.required_by {
                reverse_deps
                    .entry(dep_id.clone())
                    .or_default()
                    .insert(item.id.clone());
            }
        }

        // 2. Identify orphans: items installed as dependencies (!is_top_level) whose parents no longer exist
        let mut orphans = HashSet::new();
        for item in items.iter() {
            if !item.is_top_level {
                let has_parents = reverse_deps
                    .get(&item.id)
                    .map(|parents| !parents.is_empty())
                    .unwrap_or(false);

                if !has_parents {
                    orphans.insert(item.id.clone());
                }
            }
        }

        // 3. Update required_by on each item from our resolved inverted index
        for item in items.iter_mut() {
            if let Some(parents) = reverse_deps.get(&item.id) {
                item.required_by = parents.iter().cloned().collect();
            } else if !item.is_top_level {
                item.required_by.clear();
            }
        }

        Self {
            reverse_deps,
            orphans,
        }
    }

    /// Checks if a package is an orphan dependency
    #[allow(dead_code)]
    pub fn is_orphan(&self, item_id: &str) -> bool {
        self.orphans.contains(item_id)
    }

    /// Returns list of all detected orphan package IDs
    #[allow(dead_code)]
    pub fn orphan_count(&self) -> usize {
        self.orphans.len()
    }
}
