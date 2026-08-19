use anyhow::Result;
use crate::models::InventoryItem;

/// Core interface for discovering and parsing packages deterministically.
///
/// Implementors must adhere to:
/// 1. Strict Read-Only Operations: Never mutate or delete filesystem state during `collect()`.
/// 2. Graceful Degradation: If a tool is not installed, `is_available()` returns false,
///    allowing the system to skip it without failing the overall inventory audit.
/// 3. Thread Safety: Trait bounds `Send + Sync` allow parallel execution across threads.
pub trait Collector: Send + Sync {
    /// Human-readable identifier for this collector (e.g. "homebrew", "mise", "cargo")
    fn name(&self) -> &'static str;

    /// Checks whether the underlying package manager or directory exists on the system.
    fn is_available(&self) -> bool;

    /// Deterministically collects and parses all installed packages into normalized InventoryItems.
    fn collect(&self) -> Result<Vec<InventoryItem>>;
}
