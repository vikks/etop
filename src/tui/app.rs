use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use anyhow::Result;

use crate::cleanup::{HistoryStore, ScriptGenerator};
use crate::models::{format_bytes, Category, InventoryItem, PackageTombstone};
use crate::collectors::collect_all_parallel;
use crate::engine::{DependencyGraph, FsAuditor, TaxonomyEngine};

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub enum AuditEvent {
    StageUpdate {
        stage_name: &'static str,
        discovered_count: usize,
    },
    IncrementalBatch {
        items: Vec<InventoryItem>,
    },
    AuditComplete {
        final_items: Vec<InventoryItem>,
        elapsed_ms: u128,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    FilterMenu,
    EcosystemMenu,
    CategoryMenu,
    SortMenu,
    MarkMenu,
    Searching,
    HistoryView,
    HelpModal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    SizeDesc,
    NameAsc,
    LastUsedDesc,
    SourceAsc,
    CategoryAsc,
}

impl SortOrder {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SizeDesc     => "Disk Size (Largest first)",
            Self::NameAsc      => "Package Name (A-Z)",
            Self::LastUsedDesc => "Inactivity (Longest unused first)",
            Self::SourceAsc    => "Source Ecosystem (A-Z)",
            Self::CategoryAsc  => "Category Domain (A-Z)",
        }
    }
}

pub struct App {
    pub items: Vec<InventoryItem>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub marked_for_prune: HashSet<String>,
    pub active_ecosystems: HashSet<String>,
    pub active_categories: HashSet<Category>,
    pub only_orphans: bool,
    pub only_caches: bool,
    pub only_top_level: bool,
    pub min_inactivity_days: Option<u64>,
    pub search_query: String,
    pub active_sort: SortOrder,
    pub input_mode: InputMode,
    pub status_message: Option<(String, bool)>, // (message, is_success)

    // History / Tombstone state
    pub tombstones: Vec<PackageTombstone>,
    pub selected_tombstone_idx: usize,

    // Loading & Streaming State
    pub is_loading: bool,
    pub spin_idx: usize,
    pub current_stage: &'static str,
    pub rx: Receiver<AuditEvent>,
}

impl App {
    /// Instantly initializes the App skeleton and spawns the background audit worker thread
    pub fn new_streaming() -> Self {
        let (tx, rx) = mpsc::channel();

        // Spawn non-blocking background audit worker
        std::thread::spawn(move || {
            let start = std::time::Instant::now();

            let _ = tx.send(AuditEvent::StageUpdate {
                stage_name: "Scanning package managers (Brew, Mise, Cargo, Gems, NPM, UV, Go, Apps)...",
                discovered_count: 0,
            });

            // 1. Gather all active collectors concurrently in parallel
            let mut items = collect_all_parallel();
            let discovered_count = items.len();

            let _ = tx.send(AuditEvent::IncrementalBatch {
                items: items.clone(),
            });

            let _ = tx.send(AuditEvent::StageUpdate {
                stage_name: "Auditing APFS real disk usage & POSIX access timestamps...",
                discovered_count,
            });

            // 2. Parallel Filesystem & Metadata Audit (Sizing & Timestamps)
            FsAuditor::audit_all(&mut items);

            let _ = tx.send(AuditEvent::StageUpdate {
                stage_name: "Classifying domain categories & resolving dependency topology...",
                discovered_count,
            });

            // 3. Deterministic Taxonomy Categorization
            let taxonomy = TaxonomyEngine::load_or_default();
            for item in items.iter_mut() {
                item.category = taxonomy.classify(&item.name, &item.id);
            }

            // 4. Dependency Graph & Orphan Resolution
            let _dep_graph = DependencyGraph::resolve(&mut items);

            // 5. Complete!
            let elapsed_ms = start.elapsed().as_millis();
            let _ = tx.send(AuditEvent::AuditComplete {
                final_items: items,
                elapsed_ms,
            });
        });

        let mut app = Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            marked_for_prune: HashSet::new(),
            active_ecosystems: HashSet::new(),
            active_categories: HashSet::new(),
            only_orphans: false,
            only_caches: false,
            only_top_level: false,
            min_inactivity_days: None,
            search_query: String::new(),
            active_sort: SortOrder::SizeDesc,
            input_mode: InputMode::Normal,
            status_message: Some(("Starting real-time software audit...".to_string(), true)),
            tombstones: Vec::new(),
            selected_tombstone_idx: 0,
            is_loading: true,
            spin_idx: 0,
            current_stage: "Initializing collectors & discovery engines...",
            rx,
        };
        app.reapply_filters();
        app
    }

    pub fn tick_events(&mut self) {
        self.spin_idx = (self.spin_idx + 1) % SPINNER_FRAMES.len();

        while let Ok(event) = self.rx.try_recv() {
            match event {
                AuditEvent::StageUpdate { stage_name, discovered_count } => {
                    self.current_stage = stage_name;
                    if discovered_count > 0 {
                        self.status_message = Some((format!("Discovered {} packages so far...", discovered_count), true));
                    }
                }
                AuditEvent::IncrementalBatch { items } => {
                    self.items = items;
                    self.reapply_filters();
                }
                AuditEvent::AuditComplete { final_items, elapsed_ms } => {
                    let total = final_items.len();
                    self.items = final_items;
                    self.is_loading = false;
                    self.reapply_filters();
                    self.status_message = Some((format!("✓ Complete! Audited {} packages in {}ms", total, elapsed_ms), true));
                }
            }
        }
    }

    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spin_idx]
    }

    pub fn reapply_filters(&mut self) {
        let query = self.search_query.trim().to_lowercase();

        let mut matched: Vec<usize> = self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                // Multi-ecosystem filter
                if !self.active_ecosystems.is_empty() {
                    let matches_any = self.active_ecosystems.iter().any(|eco| item.matches_ecosystem(eco));
                    if !matches_any {
                        return false;
                    }
                }
                // Multi-category domain filter
                if !self.active_categories.is_empty() {
                    if !self.active_categories.contains(&item.category) {
                        return false;
                    }
                }
                // Orphan filter
                if self.only_orphans && !item.is_orphan() {
                    return false;
                }
                // Cache filter
                if self.only_caches && !item.is_cache() {
                    return false;
                }
                // Top level filter
                if self.only_top_level && !item.is_top_level {
                    return false;
                }
                // Inactivity filter
                if let Some(min_days) = self.min_inactivity_days {
                    if item.inactivity_days().unwrap_or(0) < min_days as i64 {
                        return false;
                    }
                }
                // Live search query filter
                if !query.is_empty() {
                    let matches_name = item.name.to_lowercase().contains(&query);
                    let matches_id = item.id.to_lowercase().contains(&query);
                    let matches_cat = item.category.as_str().to_lowercase().contains(&query);
                    let matches_src = item.source.label().to_lowercase().contains(&query);
                    if !matches_name && !matches_id && !matches_cat && !matches_src {
                        return false;
                    }
                }
                true
            })
            .map(|(idx, _)| idx)
            .collect();

        // Sort filtered items
        match self.active_sort {
            SortOrder::SizeDesc => {
                matched.sort_by(|&a, &b| self.items[b].size_bytes.cmp(&self.items[a].size_bytes));
            }
            SortOrder::NameAsc => {
                matched.sort_by(|&a, &b| self.items[a].name.to_lowercase().cmp(&self.items[b].name.to_lowercase()));
            }
            SortOrder::LastUsedDesc => {
                matched.sort_by(|&a, &b| {
                    let days_a = self.items[a].inactivity_days().unwrap_or(0);
                    let days_b = self.items[b].inactivity_days().unwrap_or(0);
                    days_b.cmp(&days_a).then_with(|| self.items[b].size_bytes.cmp(&self.items[a].size_bytes))
                });
            }
            SortOrder::SourceAsc => {
                matched.sort_by(|&a, &b| self.items[a].source.cmp(&self.items[b].source));
            }
            SortOrder::CategoryAsc => {
                matched.sort_by(|&a, &b| self.items[a].category.cmp(&self.items[b].category));
            }
        }

        self.filtered_indices = matched;

        if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len().saturating_sub(1);
        }
    }

    pub fn next(&mut self) {
        if self.input_mode == InputMode::HistoryView {
            if !self.tombstones.is_empty() {
                if self.selected_tombstone_idx + 1 < self.tombstones.len() {
                    self.selected_tombstone_idx += 1;
                } else {
                    self.selected_tombstone_idx = 0;
                }
            }
            return;
        }

        if !self.filtered_indices.is_empty() {
            if self.selected_index + 1 < self.filtered_indices.len() {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
        }
    }

    pub fn previous(&mut self) {
        if self.input_mode == InputMode::HistoryView {
            if !self.tombstones.is_empty() {
                if self.selected_tombstone_idx > 0 {
                    self.selected_tombstone_idx -= 1;
                } else {
                    self.selected_tombstone_idx = self.tombstones.len().saturating_sub(1);
                }
            }
            return;
        }

        if !self.filtered_indices.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.filtered_indices.len().saturating_sub(1);
            }
        }
    }

    pub fn page_down(&mut self) {
        if self.input_mode == InputMode::HistoryView {
            if !self.tombstones.is_empty() {
                self.selected_tombstone_idx = (self.selected_tombstone_idx + 5).min(self.tombstones.len() - 1);
            }
            return;
        }

        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 10).min(self.filtered_indices.len() - 1);
        }
    }

    pub fn page_up(&mut self) {
        if self.input_mode == InputMode::HistoryView {
            if !self.tombstones.is_empty() {
                self.selected_tombstone_idx = self.selected_tombstone_idx.saturating_sub(5);
            }
            return;
        }

        if !self.filtered_indices.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(10);
        }
    }

    pub fn toggle_mark(&mut self) {
        if let Some(&item_idx) = self.filtered_indices.get(self.selected_index) {
            let id = self.items[item_idx].id.clone();
            if self.marked_for_prune.contains(&id) {
                self.marked_for_prune.remove(&id);
            } else {
                self.marked_for_prune.insert(id);
            }
        }
    }

    pub fn mark_all_visible(&mut self) {
        let mut count = 0;
        for &idx in &self.filtered_indices {
            let item = &self.items[idx];
            if item.uninstall_cmd.is_some() {
                self.marked_for_prune.insert(item.id.clone());
                count += 1;
            }
        }
        self.status_message = Some((format!("Marked {} visible packages", count), true));
    }

    pub fn mark_visible_orphans(&mut self) {
        let mut count = 0;
        for &idx in &self.filtered_indices {
            let item = &self.items[idx];
            if item.is_orphan() && item.uninstall_cmd.is_some() {
                self.marked_for_prune.insert(item.id.clone());
                count += 1;
            }
        }
        self.status_message = Some((format!("Marked {} visible orphan dependencies", count), true));
    }

    pub fn mark_visible_caches(&mut self) {
        let mut count = 0;
        for &idx in &self.filtered_indices {
            let item = &self.items[idx];
            if item.is_cache() && item.uninstall_cmd.is_some() {
                self.marked_for_prune.insert(item.id.clone());
                count += 1;
            }
        }
        self.status_message = Some((format!("Marked {} visible caches & dangling layers", count), true));
    }

    pub fn clear_marks(&mut self) {
        self.marked_for_prune.clear();
        self.status_message = Some(("Cleared all selections".to_string(), true));
    }

    pub fn toggle_ecosystem(&mut self, eco: &str) {
        let key = eco.to_string();
        if self.active_ecosystems.contains(&key) {
            self.active_ecosystems.remove(&key);
        } else {
            self.active_ecosystems.insert(key);
        }
        self.reapply_filters();
        self.status_message = Some((format!("Active Ecosystems: {}", self.ecosystems_summary()), true));
    }

    pub fn clear_ecosystems(&mut self) {
        self.active_ecosystems.clear();
        self.reapply_filters();
        self.status_message = Some(("Active Ecosystems: ALL".to_string(), true));
    }

    pub fn ecosystems_summary(&self) -> String {
        if self.active_ecosystems.is_empty() {
            "ALL".to_string()
        } else {
            let mut list: Vec<String> = self.active_ecosystems.iter().map(|s| s.to_uppercase()).collect();
            list.sort();
            list.join("+")
        }
    }

    pub fn toggle_category(&mut self, cat: Category) {
        if self.active_categories.contains(&cat) {
            self.active_categories.remove(&cat);
        } else {
            self.active_categories.insert(cat);
        }
        self.reapply_filters();
        self.status_message = Some((format!("Active Domains: {}", self.categories_summary()), true));
    }

    pub fn clear_categories(&mut self) {
        self.active_categories.clear();
        self.reapply_filters();
        self.status_message = Some(("Active Domains: ALL".to_string(), true));
    }

    pub fn categories_summary(&self) -> String {
        if self.active_categories.is_empty() {
            "ALL".to_string()
        } else {
            let mut list: Vec<String> = self.active_categories.iter().map(|c| c.as_str().to_string()).collect();
            list.sort();
            list.join("+")
        }
    }

    pub fn toggle_orphans(&mut self) {
        self.only_orphans = !self.only_orphans;
        self.reapply_filters();
        let status = if self.only_orphans { "ON" } else { "OFF" };
        self.status_message = Some((format!("Orphan Filter: {}", status), true));
    }

    pub fn toggle_caches(&mut self) {
        self.only_caches = !self.only_caches;
        self.reapply_filters();
        let status = if self.only_caches { "ON" } else { "OFF" };
        self.status_message = Some((format!("Cache Filter: {}", status), true));
    }

    pub fn toggle_top_level(&mut self) {
        self.only_top_level = !self.only_top_level;
        self.reapply_filters();
        let status = if self.only_top_level { "ON" } else { "OFF" };
        self.status_message = Some((format!("Top-Level Filter: {}", status), true));
    }

    pub fn cycle_inactivity(&mut self) {
        let thresholds = [None, Some(30), Some(90), Some(180), Some(365)];
        let current_pos = thresholds.iter().position(|t| *t == self.min_inactivity_days).unwrap_or(0);
        let next_pos = (current_pos + 1) % thresholds.len();
        self.min_inactivity_days = thresholds[next_pos];
        self.reapply_filters();

        let label = match self.min_inactivity_days {
            Some(days) => format!("Inactivity Filter: >{} days", days),
            None => "Inactivity Filter: ALL".to_string(),
        };
        self.status_message = Some((label, true));
    }

    pub fn set_sort(&mut self, sort: SortOrder) {
        self.active_sort = sort;
        self.reapply_filters();
        self.status_message = Some((format!("Sort: {}", sort.label()), true));
    }

    pub fn clear_filters(&mut self) {
        self.active_ecosystems.clear();
        self.active_categories.clear();
        self.only_orphans = false;
        self.only_caches = false;
        self.only_top_level = false;
        self.min_inactivity_days = None;
        self.search_query.clear();
        self.reapply_filters();
        self.status_message = Some(("Cleared all filters and search".to_string(), true));
    }

    pub fn current_selected_item(&self) -> Option<&InventoryItem> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.items.get(idx))
    }

    pub fn open_history(&mut self) {
        let store = HistoryStore::new();
        if let Ok(tombstones) = store.list_all_tombstones() {
            self.tombstones = tombstones;
            self.selected_tombstone_idx = 0;
            self.input_mode = InputMode::HistoryView;
            self.status_message = Some((format!("Loaded {} uninstalled package tombstones", self.tombstones.len()), true));
        } else {
            self.status_message = Some(("Failed to load history store".to_string(), false));
        }
    }

    pub fn generate_scripts(&mut self) -> Result<()> {
        let items_to_prune: Vec<InventoryItem> = if !self.marked_for_prune.is_empty() {
            self.items
                .iter()
                .filter(|i| self.marked_for_prune.contains(&i.id) && i.uninstall_cmd.is_some())
                .cloned()
                .collect()
        } else {
            self.filtered_indices
                .iter()
                .map(|&idx| &self.items[idx])
                .filter(|i| (i.is_orphan() || i.is_cache()) && i.uninstall_cmd.is_some())
                .cloned()
                .collect()
        };

        if items_to_prune.is_empty() {
            self.status_message = Some(("No items selected or orphan/cache to prune".to_string(), false));
            return Ok(());
        }

        let total_reclaim: u64 = items_to_prune.iter().map(|i| i.size_bytes).sum();
        let (clean_path, _rollback_path, _manifest_path) = ScriptGenerator::generate_and_save(&items_to_prune, Path::new("."))?;

        let msg = format!(
            "Saved {} & manifest ({} items, {}). Run 'bash {}' to clean",
            clean_path.file_name().unwrap_or_default().to_string_lossy(),
            items_to_prune.len(),
            format_bytes(total_reclaim),
            clean_path.file_name().unwrap_or_default().to_string_lossy()
        );
        self.status_message = Some((msg, true));
        Ok(())
    }
}
