use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use serde_json::Value;

use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct CargoCollector {
    cargo_home: PathBuf,
}

impl CargoCollector {
    pub fn new() -> Self {
        let cargo_home = if let Ok(home) = std::env::var("CARGO_HOME") {
            PathBuf::from(home)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".cargo")
        } else {
            PathBuf::from("/tmp")
        };
        Self { cargo_home }
    }
}

impl Collector for CargoCollector {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn is_available(&self) -> bool {
        self.cargo_home.join("bin").exists() || self.cargo_home.join(".crates2.json").exists()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();
        let crates_json_path = self.cargo_home.join(".crates2.json");

        if crates_json_path.exists() {
            let content = fs::read_to_string(&crates_json_path)
                .with_context(|| format!("Failed to read {:?}", crates_json_path))?;

            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                if let Some(installs) = parsed.get("installs").and_then(|i| i.as_object()) {
                    for (key, val) in installs {
                        // Key is typically "crate-name 0.1.0 (registry+...)"
                        let parts: Vec<&str> = key.split_whitespace().collect();
                        let (name, version) = if parts.len() >= 2 {
                            (parts[0].to_string(), Some(parts[1].to_string()))
                        } else {
                            (key.clone(), None)
                        };

                        let bins: Vec<String> = val.get("bins")
                            .and_then(|b| b.as_array())
                            .map(|arr| {
                                arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                            })
                            .unwrap_or_default();

                        let primary_bin = bins.first().cloned().unwrap_or_else(|| name.clone());
                        let bin_path = self.cargo_home.join("bin").join(&primary_bin);

                        let id = format!("cargo:{}", name);
                        let uninstall_cmd = Some(format!("cargo uninstall {}", name));
                        let reinstall_cmd = Some(format!("cargo install {}", name));

                        items.push(InventoryItem {
                            id,
                            name: name.clone(),
                            source: PackageSource::CargoBin,
                            version,
                            install_path: bin_path,
                            size_bytes: 0,
                            last_accessed_at: None,
                            last_modified_at: None,
                            is_top_level: true,
                            required_by: Vec::new(),
                            category: Category::CliDeveloperTools,
                            uninstall_cmd,
                            reinstall_cmd,
                        });
                    }
                }
            }
        }

        Ok(items)
    }
}
