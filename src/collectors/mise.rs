use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use serde_json::Value;

use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct MiseCollector {
    mise_path: Option<PathBuf>,
}

impl MiseCollector {
    pub fn new() -> Self {
        let mise_path = which_binary("mise");
        Self { mise_path }
    }
}

impl Collector for MiseCollector {
    fn name(&self) -> &'static str {
        "mise"
    }

    fn is_available(&self) -> bool {
        self.mise_path.is_some()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mise_bin = match &self.mise_path {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        // Deterministic command: mise ls --json
        let output = Command::new(mise_bin)
            .args(["ls", "--json"])
            .output()
            .with_context(|| "Failed to execute 'mise ls --json'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise ls failed: {}", stderr);
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: Value = serde_json::from_str(&json_str)
            .with_context(|| "Failed to parse JSON output from Mise")?;

        let mut items = Vec::new();

        if let Some(obj) = parsed.as_object() {
            for (plugin, versions) in obj {
                if let Some(versions_arr) = versions.as_array() {
                    for v_obj in versions_arr {
                        let version = v_obj.get("version").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let installed = v_obj.get("installed").and_then(|i| i.as_bool()).unwrap_or(true);
                        let is_active = v_obj.get("active").and_then(|a| a.as_bool()).unwrap_or(false);

                        if !installed || version.is_empty() {
                            continue;
                        }

                        let install_path = if let Some(path_str) = v_obj.get("install_path").and_then(|p| p.as_str()) {
                            PathBuf::from(path_str)
                        } else {
                            dirs_fallback().join("installs").join(plugin).join(&version)
                        };

                        let id = format!("mise:{}@{}", plugin, version);
                        let name = format!("{} ({})", plugin, version);
                        let uninstall_cmd = Some(format!("mise uninstall {}@{}", plugin, version));
                        let reinstall_cmd = Some(format!("mise use -g {}@{}", plugin, version));

                        items.push(InventoryItem {
                            id,
                            name,
                            source: PackageSource::MiseRuntime,
                            version: Some(version),
                            install_path,
                            size_bytes: 0,
                            last_accessed_at: None,
                            last_modified_at: None,
                            is_top_level: is_active,
                            required_by: Vec::new(),
                            category: Category::LanguagesAndRuntimes,
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

fn dirs_fallback() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/share/mise")
    } else {
        PathBuf::from("/tmp")
    }
}

fn which_binary(name: &str) -> Option<PathBuf> {
    let common_paths = [
        format!("/opt/homebrew/bin/{}", name),
        format!("/usr/local/bin/{}", name),
        format!("/usr/bin/{}", name),
    ];

    for p in &common_paths {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }

    None
}
