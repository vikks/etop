use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::Deserialize;

use crate::models::Category;

#[derive(Debug, Deserialize)]
struct TaxonomyConfigFile {
    categories: HashMap<String, Vec<String>>,
}

/// Deterministic Rule-Based Taxonomy Classification Engine
pub struct TaxonomyEngine {
    lookup: HashMap<String, Category>,
}

impl TaxonomyEngine {
    /// Initializes taxonomy engine from rules/taxonomy.toml or embedded fallback defaults
    pub fn load_or_default() -> Self {
        let rules_path = Path::new("rules/taxonomy.toml");
        if rules_path.exists() {
            if let Ok(content) = fs::read_to_string(rules_path) {
                if let Ok(config) = toml::from_str::<TaxonomyConfigFile>(&content) {
                    return Self::from_config(config);
                }
            }
        }

        Self::default_engine()
    }

    fn from_config(config: TaxonomyConfigFile) -> Self {
        let mut lookup = HashMap::new();

        for (category_name, keywords) in config.categories {
            let cat = parse_category_string(&category_name);
            for kw in keywords {
                lookup.insert(kw.to_lowercase(), cat.clone());
            }
        }

        Self { lookup }
    }

    fn default_engine() -> Self {
        let mut lookup = HashMap::new();

        let defaults = [
            ("ruby", Category::LanguagesAndRuntimes),
            ("rust", Category::LanguagesAndRuntimes),
            ("go", Category::LanguagesAndRuntimes),
            ("python", Category::LanguagesAndRuntimes),
            ("node", Category::LanguagesAndRuntimes),
            ("bun", Category::LanguagesAndRuntimes),
            ("deno", Category::LanguagesAndRuntimes),
            ("postgresql", Category::DatabasesAndStorage),
            ("postgres", Category::DatabasesAndStorage),
            ("redis", Category::DatabasesAndStorage),
            ("sqlite", Category::DatabasesAndStorage),
            ("ripgrep", Category::CliDeveloperTools),
            ("fd", Category::CliDeveloperTools),
            ("bat", Category::CliDeveloperTools),
            ("fzf", Category::CliDeveloperTools),
            ("jq", Category::CliDeveloperTools),
            ("gh", Category::CliDeveloperTools),
            ("tmux", Category::CliDeveloperTools),
            ("neovim", Category::CliDeveloperTools),
            ("docker", Category::InfrastructureAndCloud),
            ("kubectl", Category::InfrastructureAndCloud),
            ("terraform", Category::InfrastructureAndCloud),
            ("mise", Category::BuildAndPackageManagers),
            ("cargo", Category::BuildAndPackageManagers),
            ("brew", Category::BuildAndPackageManagers),
            ("uv", Category::BuildAndPackageManagers),
        ];

        for (kw, cat) in defaults {
            lookup.insert(kw.to_string(), cat);
        }

        Self { lookup }
    }

    /// Classifies an item's name and ID deterministically
    pub fn classify(&self, name: &str, id: &str) -> Category {
        let norm_name = name.to_lowercase();
        let norm_id = id.to_lowercase();

        // 1. Exact name match
        if let Some(cat) = self.lookup.get(&norm_name) {
            return cat.clone();
        }

        // 2. Token prefix matching (e.g., "postgresql@14" -> "postgresql")
        for token in norm_name.split(['@', '-', '_', ' ']) {
            if let Some(cat) = self.lookup.get(token) {
                return cat.clone();
            }
        }

        // 3. Check ID tokens
        for token in norm_id.split([':', '@', '-', '_']) {
            if let Some(cat) = self.lookup.get(token) {
                return cat.clone();
            }
        }

        Category::Uncategorized
    }
}

fn parse_category_string(s: &str) -> Category {
    match s.trim().to_lowercase().as_str() {
        "languages & runtimes" | "languages and runtimes" => Category::LanguagesAndRuntimes,
        "databases & storage" | "databases and storage"   => Category::DatabasesAndStorage,
        "cli developer tools" | "cli tools"               => Category::CliDeveloperTools,
        "infrastructure & cloud" | "cloud & infra"        => Category::InfrastructureAndCloud,
        "build & package managers" | "package managers"   => Category::BuildAndPackageManagers,
        "gui apps & media" | "gui apps"                   => Category::GuiAppsAndMedia,
        "build artifacts & caches" | "caches"             => Category::BuildArtifactsAndCaches,
        "system & libraries" | "system libraries"         => Category::SystemAndLibraries,
        _ => Category::Uncategorized,
    }
}
