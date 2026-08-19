use std::path::PathBuf;
use std::process::Command;
use anyhow::Result;
use serde_json::Value;

use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct CacheCollector {
    home_dir: PathBuf,
}

impl CacheCollector {
    pub fn new() -> Self {
        let home_dir = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from("/tmp")
        };
        Self { home_dir }
    }
}

impl Collector for CacheCollector {
    fn name(&self) -> &'static str {
        "build_caches_and_docker"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();

        // 1. Cargo Registry Cache
        let cargo_cache = self.home_dir.join(".cargo/registry/cache");
        if cargo_cache.exists() {
            items.push(InventoryItem {
                id: "cache:cargo-registry".to_string(),
                name: "Cargo Registry Download Cache".to_string(),
                source: PackageSource::BuildCache,
                version: None,
                install_path: cargo_cache,
                size_bytes: 0,
                last_accessed_at: None,
                last_modified_at: None,
                is_top_level: true,
                required_by: Vec::new(),
                category: Category::BuildArtifactsAndCaches,
                uninstall_cmd: Some("cargo cache --autoclean 2>/dev/null || rm -rf ~/.cargo/registry/cache/*".to_string()),
                reinstall_cmd: None,
            });
        }

        // 2. Homebrew Download Cache
        let brew_cache = self.home_dir.join("Library/Caches/Homebrew");
        if brew_cache.exists() {
            items.push(InventoryItem {
                id: "cache:brew-downloads".to_string(),
                name: "Homebrew Bottle Download Cache".to_string(),
                source: PackageSource::BuildCache,
                version: None,
                install_path: brew_cache,
                size_bytes: 0,
                last_accessed_at: None,
                last_modified_at: None,
                is_top_level: true,
                required_by: Vec::new(),
                category: Category::BuildArtifactsAndCaches,
                uninstall_cmd: Some("brew cleanup --prune=all".to_string()),
                reinstall_cmd: None,
            });
        }

        // 3. UV / Python Cache
        let uv_cache = self.home_dir.join(".cache/uv");
        if uv_cache.exists() {
            items.push(InventoryItem {
                id: "cache:uv-wheels".to_string(),
                name: "UV Python Wheels & Source Cache".to_string(),
                source: PackageSource::BuildCache,
                version: None,
                install_path: uv_cache,
                size_bytes: 0,
                last_accessed_at: None,
                last_modified_at: None,
                is_top_level: true,
                required_by: Vec::new(),
                category: Category::BuildArtifactsAndCaches,
                uninstall_cmd: Some("uv cache clean".to_string()),
                reinstall_cmd: None,
            });
        }

        // 4. Go Module Cache
        let go_mod_cache = self.home_dir.join("go/pkg/mod/cache");
        if go_mod_cache.exists() {
            items.push(InventoryItem {
                id: "cache:go-modcache".to_string(),
                name: "Go Module Download Cache".to_string(),
                source: PackageSource::BuildCache,
                version: None,
                install_path: go_mod_cache,
                size_bytes: 0,
                last_accessed_at: None,
                last_modified_at: None,
                is_top_level: true,
                required_by: Vec::new(),
                category: Category::BuildArtifactsAndCaches,
                uninstall_cmd: Some("go clean -modcache".to_string()),
                reinstall_cmd: None,
            });
        }

        // 5. Docker Reclaimable Space (if docker daemon is responding)
        if let Ok(output) = Command::new("docker").args(["system", "df", "--format", "{{json .}}"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                        let df_type = parsed.get("Type").and_then(|t| t.as_str()).unwrap_or("");
                        let reclaimable = parsed.get("Reclaimable").and_then(|r| r.as_str()).unwrap_or("");

                        if !reclaimable.starts_with("0B") && !reclaimable.is_empty() {
                            let (id, uninstall_cmd) = match df_type {
                                "Build Cache" => (
                                    "docker:build-cache".to_string(),
                                    Some("docker builder prune -f".to_string()),
                                ),
                                "Images" => (
                                    "docker:dangling-images".to_string(),
                                    Some("docker image prune -f".to_string()),
                                ),
                                "Containers" => (
                                    "docker:stopped-containers".to_string(),
                                    Some("docker container prune -f".to_string()),
                                ),
                                _ => continue,
                            };

                            items.push(InventoryItem {
                                id,
                                name: format!("Docker Reclaimable {}", df_type),
                                source: PackageSource::DockerImage,
                                version: None,
                                install_path: PathBuf::from("/var/run/docker.sock"),
                                size_bytes: 0,
                                last_accessed_at: None,
                                last_modified_at: None,
                                is_top_level: true,
                                required_by: Vec::new(),
                                category: Category::BuildArtifactsAndCaches,
                                uninstall_cmd,
                                reinstall_cmd: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(items)
    }
}
