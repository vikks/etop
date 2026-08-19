use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use anyhow::Result;

use crate::models::{PackageTombstone, TombstoneManifest};

pub struct HistoryStore {
    base_dir: PathBuf,
}

impl HistoryStore {
    pub fn new() -> Self {
        let base_dir = if let Ok(home) = std::env::var("HOME") {
            let modern = PathBuf::from(&home).join(".local/share/etop");
            let legacy = PathBuf::from(&home).join(".local/share/mac-cleanup");
            if !modern.exists() && legacy.exists() {
                legacy
            } else {
                modern
            }
        } else {
            PathBuf::from("/tmp/etop")
        };
        Self { base_dir }
    }

    pub fn tombstones_dir(&self) -> PathBuf {
        self.base_dir.join("tombstones")
    }

    pub fn history_log_path(&self) -> PathBuf {
        self.base_dir.join("history.jsonl")
    }

    /// Persists a cleanup manifest to the tombstones directory and appends to the history log
    pub fn record_manifest(&self, manifest: &TombstoneManifest) -> Result<PathBuf> {
        let t_dir = self.tombstones_dir();
        fs::create_dir_all(&t_dir)?;

        let manifest_file = t_dir.join(format!("{}.json", manifest.manifest_id));
        let json_data = serde_json::to_string_pretty(manifest)?;
        fs::write(&manifest_file, json_data)?;

        // Append line-delimited records to history.jsonl
        let log_path = self.history_log_path();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        for tombstone in &manifest.packages {
            let line = serde_json::to_string(tombstone)?;
            writeln!(file, "{}", line)?;
        }

        Ok(manifest_file)
    }

    /// Reads all archived package tombstones from history.jsonl
    pub fn list_all_tombstones(&self) -> Result<Vec<PackageTombstone>> {
        let log_path = self.history_log_path();
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&log_path)?;
        let reader = BufReader::new(file);
        let mut tombstones = Vec::new();

        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Ok(t) = serde_json::from_str::<PackageTombstone>(trimmed) {
                    tombstones.push(t);
                }
            }
        }

        // Return sorted with latest removals first
        tombstones.sort_by(|a, b| b.removed_at.cmp(&a.removed_at));
        Ok(tombstones)
    }

    /// Searches for a specific tombstone by package ID or name
    pub fn find_tombstone(&self, query: &str) -> Result<Option<PackageTombstone>> {
        let all = self.list_all_tombstones()?;
        let q = query.to_lowercase();
        for t in all {
            if t.id.to_lowercase() == q
                || t.name.to_lowercase() == q
                || t.id.to_lowercase().contains(&q)
                || t.name.to_lowercase().contains(&q)
            {
                return Ok(Some(t));
            }
        }
        Ok(None)
    }
}
