use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::Utc;

use crate::models::{format_bytes, InventoryItem, TombstoneManifest};
use super::history::HistoryStore;

pub struct ScriptGenerator;

impl ScriptGenerator {
    /// Generates executable cleanup shell script, rollback reinstall script, and persists a forensic tombstone manifest
    pub fn generate_and_save(items: &[InventoryItem], output_dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let manifest_id = format!("cleanup_{}", timestamp);

        let cleanup_filename = format!("cleanup_{}.sh", timestamp);
        let rollback_filename = format!("rollback_reinstall_{}.sh", timestamp);
        let manifest_filename = format!("cleanup_manifest_{}.json", timestamp);

        let cleanup_path = output_dir.join(&cleanup_filename);
        let rollback_path = output_dir.join(&rollback_filename);
        let manifest_path = output_dir.join(&manifest_filename);

        // 1. Build Tombstones and Manifest
        let tombstones: Vec<_> = items.iter().map(|i| i.to_tombstone()).collect();
        let total_reclaim: u64 = items.iter().map(|i| i.size_bytes).sum();

        let manifest = TombstoneManifest {
            manifest_id: manifest_id.clone(),
            created_at: Utc::now(),
            total_packages: items.len(),
            total_reclaimed_bytes: total_reclaim,
            packages: tombstones.clone(),
        };

        // 2. Write companion manifest in output dir
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&manifest_path, manifest_json)?;

        // 3. Persist to central HistoryStore (~/.local/share/mac-cleanup/tombstones/)
        let history_store = HistoryStore::new();
        let _ = history_store.record_manifest(&manifest);

        // 4. Generate Enhanced Cleanup Script
        let cleanup_content = Self::render_cleanup_script(&manifest);
        fs::write(&cleanup_path, cleanup_content)?;

        // 5. Generate Enhanced Rollback Script
        let rollback_content = Self::render_rollback_script(&manifest);
        fs::write(&rollback_path, rollback_content)?;

        // Make scripts executable (chmod +x)
        let _ = fs::set_permissions(&cleanup_path, fs::Permissions::from_mode(0o755));
        let _ = fs::set_permissions(&rollback_path, fs::Permissions::from_mode(0o755));

        Ok((cleanup_path, rollback_path, manifest_path))
    }

    fn render_cleanup_script(manifest: &TombstoneManifest) -> String {
        let mut script = String::new();
        script.push_str("#!/usr/bin/env bash\n");
        script.push_str("# =============================================================================\n");
        script.push_str("# etop: macOS Developer Ecosystem Cleanup Plan\n");
        script.push_str(&format!("# Manifest ID:           {}\n", manifest.manifest_id));
        script.push_str(&format!("# Generated At:          {}\n", manifest.created_at.to_rfc3339()));
        script.push_str(&format!("# Candidate Packages:    {}\n", manifest.total_packages));
        script.push_str(&format!("# Estimated Disk Reclaim:{}\n", format_bytes(manifest.total_reclaimed_bytes)));
        script.push_str("#\n");
        script.push_str("# SAFETY PROTOCOL:\n");
        script.push_str("#   - All user configurations (~/.config, .toml, .plist, .rc) & ENV vars are PRESERVED.\n");
        script.push_str("#   - Phase 1 executes package manager uninstalls.\n");
        script.push_str("#   - Phase 2 cleans ONLY residual log files & transient caches without touching configs.\n");
        script.push_str("#   - A full forensic snapshot is archived in ~/.local/share/etop/tombstones/\n");
        script.push_str("# =============================================================================\n\n");
        script.push_str("set -euo pipefail\n\n");

        script.push_str("echo \"🚀 Starting deterministic macOS package cleanup with etop...\"\n");
        script.push_str("echo \"📦 Packages to remove: ");
        script.push_str(&manifest.total_packages.to_string());
        script.push_str("\"\n\n");

        // Phase 1: Native Package Uninstalls
        script.push_str("# -----------------------------------------------------------------------------\n");
        script.push_str("# PHASE 1: Native Package Uninstalls\n");
        script.push_str("# -----------------------------------------------------------------------------\n\n");

        for tombstone in &manifest.packages {
            if let Some(cmd) = &tombstone.uninstall_cmd {
                script.push_str(&format!("# Package: {} ({})\n", tombstone.name, tombstone.id));
                script.push_str(&format!("# Category: {}\n", tombstone.category));
                script.push_str(&format!("# Path: {}\n", tombstone.install_path.display()));
                script.push_str(&format!("echo \"👉 [Phase 1] Removing {}...\"\n", tombstone.name));
                script.push_str(&format!("{}\n\n", cmd));
            }
        }

        // Phase 2: Targeted Log & Cache Purge
        script.push_str("# -----------------------------------------------------------------------------\n");
        script.push_str("# PHASE 2: Targeted Log Files & Transient Cache Purge\n");
        script.push_str("# (Configurations and environment settings are kept completely intact!)\n");
        script.push_str("# -----------------------------------------------------------------------------\n\n");

        let mut has_logs_to_purge = false;
        for tombstone in &manifest.packages {
            if !tombstone.log_purge_cmds.is_empty() {
                has_logs_to_purge = true;
                script.push_str(&format!("# Logs & Caches for: {}\n", tombstone.name));
                for purge_cmd in &tombstone.log_purge_cmds {
                    script.push_str(&format!("{}\n", purge_cmd));
                }
                script.push('\n');
            }
        }

        if !has_logs_to_purge {
            script.push_str("# No residual log files or caches found to purge.\n\n");
        }

        script.push_str("echo \"✨ Cleanup completed successfully!\"\n");
        script.push_str(&format!("echo \"🔄 Rollback script available: ./rollback_reinstall_{}.sh\"\n", manifest.manifest_id.strip_prefix("cleanup_").unwrap_or("")));
        script

    }

    fn render_rollback_script(manifest: &TombstoneManifest) -> String {
        let mut script = String::new();
        script.push_str("#!/usr/bin/env bash\n");
        script.push_str("# =============================================================================\n");
        script.push_str("# etop: macOS Package Rollback & Inverse Reinstall Plan\n");
        script.push_str(&format!("# Manifest ID:           {}\n", manifest.manifest_id));
        script.push_str(&format!("# Generated At:          {}\n", manifest.created_at.to_rfc3339()));
        script.push_str("#\n");
        script.push_str("# INVERSE OPERATION:\n");
        script.push_str("#   - Reinstalls all packages removed by this etop cleanup session.\n");
        script.push_str("#   - Preserved configurations and historical environment variables are documented below.\n");
        script.push_str("# =============================================================================\n\n");
        script.push_str("set -euo pipefail\n\n");

        script.push_str("echo \"🔄 Restoring and reinstalling packages...\"\n\n");

        for tombstone in &manifest.packages {
            script.push_str("# -----------------------------------------------------------------------------\n");
            script.push_str(&format!("# Package: {} ({})\n", tombstone.name, tombstone.id));
            script.push_str(&format!("# Category: {}\n", tombstone.category));
            script.push_str(&format!("# Original Path: {}\n", tombstone.install_path.display()));

            // Document preserved configs
            if !tombstone.associated_metadata.configs.is_empty() {
                script.push_str("# Preserved Config Files:\n");
                for p in &tombstone.associated_metadata.configs {
                    script.push_str(&format!("#   - {}\n", p.display()));
                }
            }

            // Document active environment variables
            if !tombstone.associated_metadata.env_vars.is_empty() {
                script.push_str("# Associated Environment Variables:\n");
                for (k, v) in &tombstone.associated_metadata.env_vars {
                    script.push_str(&format!("#   export {}='{}'\n", k, v));
                }
            }

            if let Some(reinstall) = &tombstone.reinstall_cmd {
                script.push_str(&format!("echo \"👉 Reinstalling {}...\"\n", tombstone.name));
                script.push_str(&format!("{}\n\n", reinstall));
            } else {
                script.push_str(&format!("# Manual restore required for {} (no automatic package manager reinstall command)\n\n", tombstone.name));
            }
        }

        script.push_str("echo \"✨ Rollback and reinstall completed!\"\n");
        script
    }
}
