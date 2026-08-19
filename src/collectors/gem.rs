use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use crate::models::{Category, InventoryItem, PackageSource};
use super::trait_def::Collector;

pub struct GemCollector {
    gem_bins: Vec<PathBuf>,
}

impl GemCollector {
    pub fn new() -> Self {
        let mut gem_bins = Vec::new();

        // 1. Check Mise installed Rubies
        if let Ok(home) = std::env::var("HOME") {
            let mise_rubies_dir = PathBuf::from(home).join(".local/share/mise/installs/ruby");
            if mise_rubies_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(mise_rubies_dir) {
                    for entry in entries.flatten() {
                        let gem_bin = entry.path().join("bin/gem");
                        if gem_bin.exists() {
                            gem_bins.push(gem_bin);
                        }
                    }
                }
            }
        }

        // 2. Check system gem if no Mise gems found
        if gem_bins.is_empty() {
            if let Some(system_gem) = which_binary("gem") {
                gem_bins.push(system_gem);
            }
        }

        Self { gem_bins }
    }
}

impl Collector for GemCollector {
    fn name(&self) -> &'static str {
        "gem"
    }

    fn is_available(&self) -> bool {
        !self.gem_bins.is_empty()
    }

    fn collect(&self) -> Result<Vec<InventoryItem>> {
        let mut items = Vec::new();

        for gem_bin in &self.gem_bins {
            let gem_dir = get_gem_dir(gem_bin);

            // Execute: gem list --no-details
            let output = Command::new(gem_bin)
                .args(["list", "--no-details"])
                .output()
                .with_context(|| format!("Failed to run {:?}", gem_bin))?;

            if !output.status.success() {
                continue;
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Format: "bootsnap (1.18.6)" or "bundler (default: 4.0.18)" or "racc (1.8.1, default: 1.7.3)"
                if let Some(open_paren) = trimmed.find('(') {
                    let name = trimmed[..open_paren].trim().to_string();
                    let versions_raw = trimmed[open_paren + 1..].trim_end_matches(')').trim();
                    let is_default = versions_raw.contains("default");

                    let primary_version = versions_raw
                        .split(',')
                        .next()
                        .map(|v| v.replace("default:", "").trim().to_string());

                    let id = format!("gem:{}", name);
                    let install_path = if let Some(dir) = &gem_dir {
                        dir.join("gems").join(format!("{}-{}", name, primary_version.as_deref().unwrap_or("")))
                    } else {
                        PathBuf::from(format!("/tmp/gem/{}", name))
                    };

                    let uninstall_cmd = if !is_default {
                        Some(format!("gem uninstall {}", name))
                    } else {
                        None // Default Ruby core gems should not be uninstalled
                    };

                    let reinstall_cmd = Some(format!("gem install {}", name));

                    items.push(InventoryItem {
                        id,
                        name: name.clone(),
                        source: PackageSource::Gem,
                        version: primary_version,
                        install_path,
                        size_bytes: 0,
                        last_accessed_at: None,
                        last_modified_at: None,
                        is_top_level: !is_default,
                        required_by: Vec::new(),
                        category: Category::LanguagesAndRuntimes,
                        uninstall_cmd,
                        reinstall_cmd,
                    });
                }
            }
        }

        Ok(items)
    }
}

fn get_gem_dir(gem_bin: &PathBuf) -> Option<PathBuf> {
    let output = Command::new(gem_bin)
        .args(["environment", "gemdir"])
        .output()
        .ok()?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            return Some(PathBuf::from(path_str));
        }
    }
    None
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
