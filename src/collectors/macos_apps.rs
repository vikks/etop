use std::fs;
use std::path::PathBuf;
use anyhow::Result;
use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct MacAppsCollector {
    app_dirs: Vec<PathBuf>,
}

impl MacAppsCollector {
    pub fn new() -> Self {
        let mut app_dirs = vec![PathBuf::from("/Applications")];
        if let Ok(home) = std::env::var("HOME") {
            let user_apps = PathBuf::from(home).join("Applications");
            if user_apps.exists() {
                app_dirs.push(user_apps);
            }
        }
        Self { app_dirs }
    }
}

impl Collector for MacAppsCollector {
    fn name(&self) -> &'static str {
        "macos_apps"
    }

    fn is_available(&self) -> bool {
        self.app_dirs.iter().any(|d| d.exists())
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();

        for dir in &self.app_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.extension().and_then(|e| e.to_str()) == Some("app") {
                        let file_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                        let version = get_plist_version(&path);

                        let id = format!("app:{}", file_name.to_lowercase().replace(' ', "-"));
                        let uninstall_cmd = Some(format!("rm -rf \"{}\"", path.display()));

                        items.push(InventoryItem {
                            id,
                            name: file_name.clone(),
                            source: PackageSource::MacApp,
                            version,
                            install_path: path,
                            size_bytes: 0,
                            last_accessed_at: None,
                            last_modified_at: None,
                            is_top_level: true,
                            required_by: Vec::new(),
                            category: Category::GuiAppsAndMedia,
                            uninstall_cmd,
                            reinstall_cmd: None,
                        });
                    }
                }
            }
        }

        Ok(items)
    }
}

/// Extracts version from Info.plist via fast string parsing without child process spawning
fn get_plist_version(app_path: &PathBuf) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    if let Ok(content) = fs::read_to_string(&plist_path) {
        if let Some(pos) = content.find("CFBundleShortVersionString") {
            let slice = &content[pos..];
            if let Some(string_start) = slice.find("<string>") {
                let val_start = &slice[string_start + 8..];
                if let Some(string_end) = val_start.find("</string>") {
                    let ver = val_start[..string_end].trim().to_string();
                    if !ver.is_empty() {
                        return Some(ver);
                    }
                }
            }
        }
    }
    None
}
