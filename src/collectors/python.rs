use std::path::PathBuf;
use std::process::Command;
use anyhow::Result;
use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct PythonCollector {
    uv_bin: Option<PathBuf>,
}

impl PythonCollector {
    pub fn new() -> Self {
        let uv_bin = which_binary("uv");
        Self { uv_bin }
    }
}

impl Collector for PythonCollector {
    fn name(&self) -> &'static str {
        "python_uv"
    }

    fn is_available(&self) -> bool {
        self.uv_bin.is_some()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();

        if let Some(uv) = &self.uv_bin {
            // Execute: uv tool list
            if let Ok(output) = Command::new(uv).args(["tool", "list"]).output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    let uv_tools_dir = PathBuf::from(home).join(".local/share/uv/tools");

                    for line in stdout.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('-') {
                            let parts: Vec<&str> = trimmed.split_whitespace().collect();
                            let name = parts.first().unwrap_or(&"").to_string();
                            let version = parts.get(1).map(|v| v.trim_start_matches('v').to_string());

                            if !name.is_empty() {
                                let install_path = uv_tools_dir.join(&name);
                                let id = format!("uv:tool:{}", name);
                                let uninstall_cmd = Some(format!("uv tool uninstall {}", name));
                                let reinstall_cmd = Some(format!("uv tool install {}", name));

                                items.push(InventoryItem {
                                    id,
                                    name: name.clone(),
                                    source: PackageSource::UvTool,
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
                }
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
