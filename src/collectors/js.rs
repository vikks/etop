use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use serde_json::Value;

use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct JsCollector {
    npm_bin: Option<PathBuf>,
}

impl JsCollector {
    pub fn new() -> Self {
        let npm_bin = which_binary("npm");
        Self { npm_bin }
    }
}

impl Collector for JsCollector {
    fn name(&self) -> &'static str {
        "node_npm"
    }

    fn is_available(&self) -> bool {
        self.npm_bin.is_some()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let npm = match &self.npm_bin {
            Some(bin) => bin,
            None => return Ok(Vec::new()),
        };

        // Deterministic query: npm list -g --json --depth=0
        let output = Command::new(npm)
            .args(["list", "-g", "--json", "--depth=0"])
            .output()
            .with_context(|| "Failed to execute npm list -g")?;

        let mut items = Vec::new();

        if let Ok(parsed) = serde_json::from_slice::<Value>(&output.stdout) {
            let global_prefix = get_npm_global_prefix(npm);

            if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) {
                for (name, details) in deps {
                    let version = details.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let install_path = global_prefix.join("lib/node_modules").join(name);

                    let id = format!("npm:{}", name);
                    let uninstall_cmd = Some(format!("npm uninstall -g {}", name));
                    let reinstall_cmd = Some(format!("npm install -g {}", name));

                    items.push(InventoryItem {
                        id,
                        name: name.clone(),
                        source: PackageSource::NpmGlobal,
                        version,
                        install_path,
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

        Ok(items)
    }
}

fn get_npm_global_prefix(npm_bin: &PathBuf) -> PathBuf {
    if let Ok(output) = Command::new(npm_bin).args(["config", "get", "prefix"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    PathBuf::from("/usr/local")
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
