use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use serde_json::Value;

use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct BrewCollector {
    brew_path: Option<PathBuf>,
}

impl BrewCollector {
    pub fn new() -> Self {
        let brew_path = which_binary("brew");
        Self { brew_path }
    }
}

impl Collector for BrewCollector {
    fn name(&self) -> &'static str {
        "homebrew"
    }

    fn is_available(&self) -> bool {
        self.brew_path.is_some()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let brew_bin = match &self.brew_path {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        // Deterministic command: brew info --json=v2 --installed
        let output = Command::new(brew_bin)
            .args(["info", "--json=v2", "--installed"])
            .output()
            .with_context(|| "Failed to execute 'brew info --json=v2 --installed'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("brew info failed: {}", stderr);
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: Value = serde_json::from_str(&json_str)
            .with_context(|| "Failed to parse JSON output from Homebrew")?;

        let mut items = Vec::new();
        let prefix = get_brew_prefix(brew_bin);

        // 1. Parse Formulae
        if let Some(formulae) = parsed.get("formulae").and_then(|f| f.as_array()) {
            for formula in formulae {
                let name = formula.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }

                let installed_arr = formula.get("installed").and_then(|i| i.as_array());
                let (version, is_top_level) = if let Some(installs) = installed_arr {
                    if let Some(first) = installs.first() {
                        let ver = first.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let installed_as_dep = first.get("installed_as_dependency").and_then(|d| d.as_bool()).unwrap_or(false);
                        let installed_on_req = first.get("installed_on_request").and_then(|r| r.as_bool()).unwrap_or(!installed_as_dep);
                        (ver, installed_on_req)
                    } else {
                        (None, true)
                    }
                } else {
                    (None, true)
                };

                let ver_str = version.clone().unwrap_or_else(|| "current".to_string());
                let cellar_path = prefix.join("Cellar").join(&name).join(&ver_str);
                let install_path = if cellar_path.exists() {
                    cellar_path
                } else {
                    prefix.join("opt").join(&name)
                };

                // Dependencies required by this package
                let mut required_by = Vec::new();
                if let Some(deps) = formula.get("dependencies").and_then(|d| d.as_array()) {
                    for dep in deps {
                        if let Some(dep_name) = dep.as_str() {
                            required_by.push(format!("brew:{}", dep_name));
                        }
                    }
                }

                let id = format!("brew:{}", name);
                let uninstall_cmd = Some(format!("brew uninstall {}", name));
                let reinstall_cmd = Some(format!("brew install {}", name));

                items.push(InventoryItem {
                    id,
                    name: name.clone(),
                    source: PackageSource::BrewFormula,
                    version,
                    install_path,
                    size_bytes: 0,
                    last_accessed_at: None,
                    last_modified_at: None,
                    is_top_level,
                    required_by,
                    category: Category::Uncategorized,
                    uninstall_cmd,
                    reinstall_cmd,
                });
            }
        }

        // 2. Parse Casks
        if let Some(casks) = parsed.get("casks").and_then(|c| c.as_array()) {
            for cask in casks {
                let token = cask.get("token").and_then(|t| t.as_str()).unwrap_or("").to_string();
                if token.is_empty() {
                    continue;
                }

                let version = cask.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                let caskroom_path = prefix.join("Caskroom").join(&token);

                let id = format!("brew:cask:{}", token);
                let uninstall_cmd = Some(format!("brew uninstall --cask {}", token));
                let reinstall_cmd = Some(format!("brew install --cask {}", token));

                items.push(InventoryItem {
                    id,
                    name: token.clone(),
                    source: PackageSource::BrewCask,
                    version,
                    install_path: caskroom_path,
                    size_bytes: 0,
                    last_accessed_at: None,
                    last_modified_at: None,
                    is_top_level: true, // All casks are top-level
                    required_by: Vec::new(),
                    category: Category::GuiAppsAndMedia,
                    uninstall_cmd,
                    reinstall_cmd,
                });
            }
        }

        Ok(items)
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

fn get_brew_prefix(brew_bin: &PathBuf) -> PathBuf {
    if let Ok(output) = Command::new(brew_bin).arg("--prefix").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }

    if brew_bin.starts_with("/opt/homebrew") {
        PathBuf::from("/opt/homebrew")
    } else {
        PathBuf::from("/usr/local")
    }
}
