use std::fs;
use std::path::Path;
use std::time::SystemTime;
use chrono::{DateTime, Utc};
use rayon::prelude::*;

use crate::models::InventoryItem;

/// Fast parallel filesystem and metadata auditor
pub struct FsAuditor;

impl FsAuditor {
    /// Enriches inventory items in parallel with exact disk sizes and POSIX timestamps
    ///
    /// Architectural Note on Parallelism:
    /// The outer collection `items.par_iter_mut()` is distributed across Rayon's work-stealing
    /// thread pool (one worker per CPU core). Inside each worker thread, `calculate_path_size`
    /// performs efficient sequential recursive traversal to avoid nested thread pool explosion.
    pub fn audit_all(items: &mut [InventoryItem]) {
        items.par_iter_mut().for_each(|item| {
            if item.install_path.exists() {
                // 1. Calculate real disk usage
                item.size_bytes = calculate_path_size(&item.install_path);

                // 2. Extract POSIX timestamps
                if let Ok(metadata) = fs::metadata(&item.install_path) {
                    if let Ok(mtime) = metadata.modified() {
                        item.last_modified_at = system_time_to_datetime(mtime);
                    }

                    if let Ok(atime) = metadata.accessed() {
                        item.last_accessed_at = system_time_to_datetime(atime);
                    }
                }
            }
        });
    }
}

/// Recursively calculates total disk size in bytes for a file or directory tree
pub fn calculate_path_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return 0; // Don't traverse symlinks to avoid infinite loops and duplicate counts
        }

        if metadata.is_file() {
            return metadata.len();
        }

        if metadata.is_dir() {
            let mut total: u64 = 0;
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let child_path = entry.path();
                    total += calculate_path_size(&child_path);
                }
            }
            return total;
        }
    }

    0
}

fn system_time_to_datetime(t: SystemTime) -> Option<DateTime<Utc>> {
    let duration = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
}
