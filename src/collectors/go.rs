use std::path::PathBuf;
use anyhow::Result;
use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct GoCollector {
    go_bin_dir: PathBuf,
}

impl GoCollector {
    pub fn new() -> Self {
        let go_bin_dir = if let Ok(gopath) = std::env::var("GOPATH") {
            PathBuf::from(gopath).join("bin")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join("go/bin")
        } else {
            PathBuf::from("/tmp/go/bin")
        };
        Self { go_bin_dir }
    }
}

impl Collector for GoCollector {
    fn name(&self) -> &'static str {
        "go"
    }

    fn is_available(&self) -> bool {
        self.go_bin_dir.exists()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.go_bin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if file_name.starts_with('.') {
                        continue;
                    }

                    let id = format!("go:{}", file_name);
                    let uninstall_cmd = Some(format!("rm \"{}\"", path.display()));

                    items.push(InventoryItem {
                        id,
                        name: file_name.clone(),
                        source: PackageSource::GoBin,
                        version: None,
                        install_path: path,
                        size_bytes: 0,
                        last_accessed_at: None,
                        last_modified_at: None,
                        is_top_level: true,
                        required_by: Vec::new(),
                        category: Category::CliDeveloperTools,
                        uninstall_cmd,
                        reinstall_cmd: None,
                    });
                }
            }
        }

        Ok(items)
    }
}
