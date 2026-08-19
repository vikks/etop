use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::category::Category;
use super::item::{AssociatedMetadata, PackageSource};

/// Represents an archived forensic snapshot of an uninstalled package
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageTombstone {
    pub id: String,
    pub name: String,
    pub source: PackageSource,
    pub version: Option<String>,
    pub removed_at: DateTime<Utc>,
    pub install_path: PathBuf,
    pub size_bytes: u64,
    pub last_accessed_at: Option<DateTime<Utc>>,
    pub category: Category,
    pub required_by: Vec<String>,
    pub associated_metadata: AssociatedMetadata,
    pub uninstall_cmd: Option<String>,
    pub reinstall_cmd: Option<String>,
    pub log_purge_cmds: Vec<String>,
}

/// A structured manifest containing all tombstones from a specific cleanup run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneManifest {
    pub manifest_id: String,
    pub created_at: DateTime<Utc>,
    pub total_packages: usize,
    pub total_reclaimed_bytes: u64,
    pub packages: Vec<PackageTombstone>,
}
