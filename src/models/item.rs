use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::category::Category;
use super::tombstone::PackageTombstone;

/// Represents the origin ecosystem of an installed package or tool
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PackageSource {
    BrewFormula,
    BrewCask,
    MiseRuntime,
    CargoBin,
    Gem,
    NpmGlobal,
    UvTool,
    GoBin,
    MacApp,
    BuildCache,
    DockerImage,
}

impl PackageSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::BrewFormula => "brew:formula",
            Self::BrewCask    => "brew:cask",
            Self::MiseRuntime => "mise:runtime",
            Self::CargoBin    => "cargo:bin",
            Self::Gem         => "gem:ruby",
            Self::NpmGlobal   => "npm:global",
            Self::UvTool      => "uv:python",
            Self::GoBin       => "go:bin",
            Self::MacApp      => "macos:app",
            Self::BuildCache  => "cache:build",
            Self::DockerImage => "docker:image",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::BrewFormula => "🍺",
            Self::BrewCask    => "📦",
            Self::MiseRuntime => "⚡",
            Self::CargoBin    => "🦀",
            Self::Gem         => "💎",
            Self::NpmGlobal   => "🌐",
            Self::UvTool      => "🐍",
            Self::GoBin       => "🐹",
            Self::MacApp      => "🍎",
            Self::BuildCache  => "🧹",
            Self::DockerImage => "🐳",
        }
    }

    pub fn matches_ecosystem(&self, eco: &str) -> bool {
        let q = eco.to_lowercase();
        match self {
            Self::BrewFormula | Self::BrewCask => q == "brew" || q == "homebrew",
            Self::MiseRuntime => q == "mise" || q == "runtime" || q == "runtimes",
            Self::CargoBin => q == "cargo" || q == "rust" || q == "rs",
            Self::Gem => q == "gem" || q == "ruby" || q == "rb",
            Self::NpmGlobal => q == "npm" || q == "node" || q == "js" || q == "javascript" || q == "ts",
            Self::UvTool => q == "uv" || q == "python" || q == "py" || q == "pip",
            Self::GoBin => q == "go" || q == "golang",
            Self::MacApp => q == "app" || q == "apps" || q == "macos" || q == "gui",
            Self::BuildCache => q == "cache" || q == "caches" || q == "build",
            Self::DockerImage => q == "docker" || q == "container" || q == "containers",
        }
    }
}

/// Represents the factual dependency status of an installed package
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageStatus {
    TopLevel,
    Orphan,
    Dependency { required_by_count: usize },
    BuildCache,
    ActiveRuntime,
}

impl PackageStatus {
    pub fn label(&self) -> String {
        match self {
            Self::TopLevel => "📦 Top-Level".to_string(),
            Self::Orphan => "🧹 Orphan (0 refs)".to_string(),
            Self::Dependency { required_by_count } => format!("⛓️ Dep ({} refs)", required_by_count),
            Self::BuildCache => "💾 Cache".to_string(),
            Self::ActiveRuntime => "⚡ Active Runtime".to_string(),
        }
    }
}

/// Associated files, configs, data, logs, and environment variables on macOS
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AssociatedMetadata {
    pub configs: Vec<PathBuf>,
    pub data_dirs: Vec<PathBuf>,
    pub logs: Vec<PathBuf>,
    pub caches: Vec<PathBuf>,
    pub env_vars: Vec<(String, String)>,
}

/// Normalized canonical data model representing an installed software item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: String,
    pub name: String,
    pub source: PackageSource,
    pub version: Option<String>,
    pub install_path: PathBuf,
    pub size_bytes: u64,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub last_modified_at: Option<DateTime<Utc>>,
    pub is_top_level: bool,
    pub required_by: Vec<String>,
    pub category: Category,
    pub uninstall_cmd: Option<String>,
    pub reinstall_cmd: Option<String>,
}

impl InventoryItem {
    /// Formats disk footprint into human-readable unit
    pub fn formatted_size(&self) -> String {
        format_bytes(self.size_bytes)
    }

    /// Computes inactivity in days from POSIX atime / mdls
    pub fn inactivity_days(&self) -> Option<i64> {
        self.last_accessed_at.map(|accessed| {
            let now = Utc::now();
            let duration = now.signed_duration_since(accessed);
            duration.num_days().max(0)
        })
    }

    /// Formats last access time as a human-friendly string
    pub fn formatted_last_accessed(&self) -> String {
        if let Some(days) = self.inactivity_days() {
            if days == 0 {
                "Today".to_string()
            } else if days == 1 {
                "1d ago".to_string()
            } else {
                format!("{}d ago", days)
            }
        } else {
            "Unknown".to_string()
        }
    }

    /// Returns the objective package relationship status
    pub fn status(&self) -> PackageStatus {
        if self.source == PackageSource::BuildCache || self.source == PackageSource::DockerImage {
            PackageStatus::BuildCache
        } else if self.source == PackageSource::MiseRuntime && self.is_top_level {
            PackageStatus::ActiveRuntime
        } else if !self.required_by.is_empty() {
            PackageStatus::Dependency {
                required_by_count: self.required_by.len(),
            }
        } else if !self.is_top_level {
            PackageStatus::Orphan
        } else {
            PackageStatus::TopLevel
        }
    }

    /// Returns true if this package is an unreferenced orphan dependency
    pub fn is_orphan(&self) -> bool {
        matches!(self.status(), PackageStatus::Orphan)
    }

    /// Returns true if this item is a build cache or dangling image
    pub fn is_cache(&self) -> bool {
        matches!(self.status(), PackageStatus::BuildCache)
    }

    /// Checks if item matches an ecosystem query
    pub fn matches_ecosystem(&self, eco: &str) -> bool {
        self.source.matches_ecosystem(eco)
            || (eco.to_lowercase() == "ruby" && self.id.starts_with("mise:ruby"))
            || (eco.to_lowercase() == "rust" && self.id.starts_with("cargo:"))
            || (eco.to_lowercase() == "python" && self.id.starts_with("uv:"))
            || (eco.to_lowercase() == "go" && self.id.starts_with("go:"))
            || (eco.to_lowercase() == "node" && self.id.starts_with("npm:"))
    }

    /// Dynamically probes existing configuration files, data paths, log files, caches, and environment variables
    pub fn discover_associated_metadata(&self) -> AssociatedMetadata {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/tmp"));
        let name_lower = self.name.to_lowercase();
        let name_clean = name_lower.trim_start_matches('@').replace('/', "-").replace(' ', "-");
        let name_stem = if let Some(stripped) = name_clean.split('@').next() {
            if stripped.is_empty() {
                name_clean.clone()
            } else {
                stripped.to_string()
            }
        } else {
            name_clean.clone()
        };

        if name_stem.trim().len() < 2 || name_stem == "." {
            return AssociatedMetadata::default();
        }

        let mut meta = AssociatedMetadata::default();

        // 1. Candidate Configs (Preserved - Never deleted by default)
        let candidate_configs = vec![
            home.join(format!(".config/{}", name_stem)),
            home.join(format!(".config/{}.toml", name_stem)),
            home.join(format!(".{}rc", name_stem)),
            home.join(format!(".{}", name_stem)),
            home.join(format!("Library/Preferences/{}.plist", name_stem)),
            home.join(format!("Library/Preferences/com.{}.plist", name_stem)),
            PathBuf::from(format!("/opt/homebrew/etc/{}", name_stem)),
            PathBuf::from(format!("/usr/local/etc/{}", name_stem)),
        ];
        for p in candidate_configs {
            if p.exists() && p != home && p != home.join(".config") && !meta.configs.contains(&p) {
                meta.configs.push(p.clone());

                // Check for embedded log files inside config directories
                if p.is_dir() {
                    if let Ok(entries) = fs::read_dir(&p) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if let Some(ext) = entry_path.extension() {
                                if ext == "log" && !meta.logs.contains(&entry_path) {
                                    meta.logs.push(entry_path.clone());
                                }
                            }
                            if entry_path.is_dir() && entry_path.file_name().and_then(|n| n.to_str()) == Some("logs") {
                                if !meta.logs.contains(&entry_path) {
                                    meta.logs.push(entry_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Candidate Data Dirs
        let candidate_data = vec![
            home.join(format!("Library/Application Support/{}", self.name)),
            home.join(format!("Library/Application Support/{}", name_stem)),
            home.join(format!(".local/share/{}", name_stem)),
            PathBuf::from(format!("/opt/homebrew/var/{}", name_stem)),
            PathBuf::from(format!("/usr/local/var/{}", name_stem)),
        ];
        for p in candidate_data {
            if p.exists() && p != self.install_path && p != home && !meta.data_dirs.contains(&p) {
                meta.data_dirs.push(p.clone());

                // Check for embedded log files inside application support directories
                if p.is_dir() {
                    if let Ok(entries) = fs::read_dir(&p) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if let Some(ext) = entry_path.extension() {
                                if ext == "log" && !meta.logs.contains(&entry_path) {
                                    meta.logs.push(entry_path.clone());
                                }
                            }
                            if entry_path.is_dir() && entry_path.file_name().and_then(|n| n.to_str()) == Some("logs") {
                                if !meta.logs.contains(&entry_path) {
                                    meta.logs.push(entry_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Candidate Log Files
        let candidate_logs = vec![
            home.join(format!("Library/Logs/{}", self.name)),
            home.join(format!("Library/Logs/{}", name_stem)),
            PathBuf::from(format!("/opt/homebrew/var/log/{}", name_stem)),
            PathBuf::from(format!("/usr/local/var/log/{}", name_stem)),
            PathBuf::from(format!("/var/log/{}", name_stem)),
        ];
        for p in candidate_logs {
            if p.exists() && p != home && p != home.join("Library/Logs") && !meta.logs.contains(&p) {
                meta.logs.push(p);
            }
        }

        // 4. Candidate Cache Dirs
        let candidate_caches = vec![
            home.join(format!("Library/Caches/{}", self.name)),
            home.join(format!("Library/Caches/{}", name_stem)),
            home.join(format!(".cache/{}", name_stem)),
        ];
        for p in candidate_caches {
            if p.exists() && p != self.install_path && p != home && p != home.join(".cache") && !meta.caches.contains(&p) {
                meta.caches.push(p);
            }
        }

        // 5. Relevant Environment Variables
        let upper_name = name_stem.to_uppercase().replace('-', "_");
        let all_env: Vec<(String, String)> = std::env::vars().collect();
        for (k, v) in &all_env {
            let is_match = match self.source {
                PackageSource::CargoBin => k.starts_with("CARGO_") || k.starts_with("RUST"),
                PackageSource::Gem => k.starts_with("GEM_") || k.starts_with("BUNDLE_") || k.starts_with("RUBY"),
                PackageSource::MiseRuntime => {
                    if self.id.starts_with("mise:ruby") {
                        k.starts_with("GEM_") || k.starts_with("BUNDLE_") || k.starts_with("RUBY") || k.starts_with("MISE_")
                    } else if self.id.starts_with("mise:node") {
                        k.starts_with("NODE_") || k.starts_with("NPM_") || k.starts_with("MISE_")
                    } else if self.id.starts_with("mise:python") {
                        k.starts_with("PYTHON") || k.starts_with("VIRTUAL_ENV") || k.starts_with("MISE_")
                    } else {
                        k.starts_with("MISE_")
                    }
                }
                PackageSource::NpmGlobal => k.starts_with("NODE_") || k.starts_with("NPM_") || k.starts_with("BUN_"),
                PackageSource::UvTool => k.starts_with("UV_") || k.starts_with("PYTHON") || k == "VIRTUAL_ENV",
                PackageSource::GoBin => k.starts_with("GO"),
                PackageSource::BrewFormula | PackageSource::BrewCask => k.starts_with("HOMEBREW_"),
                PackageSource::DockerImage => k.starts_with("DOCKER_"),
                _ => false,
            };

            let name_in_key = !upper_name.is_empty() && upper_name.len() >= 3 && k.contains(&upper_name);

            if is_match || name_in_key {
                if !meta.env_vars.iter().any(|(existing_k, _)| existing_k == k) {
                    meta.env_vars.push((k.clone(), v.clone()));
                }
            }
        }

        meta.env_vars.sort_by(|a, b| a.0.cmp(&b.0));
        meta
    }

    /// Generates targeted commands to purge ONLY log files (including logs inside config dirs) and caches without touching configs
    pub fn generate_targeted_log_purge_cmds(&self) -> Vec<String> {
        let meta = self.discover_associated_metadata();
        let mut cmds = Vec::new();

        for log_path in &meta.logs {
            if log_path.is_dir() {
                cmds.push(format!("rm -rf \"{}\"", log_path.display()));
            } else if log_path.is_file() {
                cmds.push(format!("rm -f \"{}\"", log_path.display()));
            }
        }

        for cache_path in &meta.caches {
            if cache_path.is_dir() {
                cmds.push(format!("rm -rf \"{}\"", cache_path.display()));
            }
        }

        cmds
    }

    /// Converts this live item into an archival forensic tombstone snapshot
    pub fn to_tombstone(&self) -> PackageTombstone {
        let associated_metadata = self.discover_associated_metadata();
        let log_purge_cmds = self.generate_targeted_log_purge_cmds();

        PackageTombstone {
            id: self.id.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            version: self.version.clone(),
            removed_at: Utc::now(),
            install_path: self.install_path.clone(),
            size_bytes: self.size_bytes,
            last_accessed_at: self.last_accessed_at,
            category: self.category.clone(),
            required_by: self.required_by.clone(),
            associated_metadata,
            uninstall_cmd: self.uninstall_cmd.clone(),
            reinstall_cmd: self.reinstall_cmd.clone(),
            log_purge_cmds,
        }
    }
}

/// Helper function to format raw bytes into human-readable binary units
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
