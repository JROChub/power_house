#![cfg(feature = "net")]

//! Migration mode helpers used to freeze mutable stake/blob writes.

use std::sync::atomic::{AtomicBool, Ordering};

static MIGRATION_FREEZE: AtomicBool = AtomicBool::new(false);

/// Refreshes migration mode from `PH_MIGRATION_MODE` and returns whether freeze is active.
pub fn refresh_migration_mode_from_env() -> bool {
    let freeze = std::env::var("PH_MIGRATION_MODE")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("freeze"))
        .unwrap_or(false);
    MIGRATION_FREEZE.store(freeze, Ordering::Relaxed);
    freeze
}

/// Returns true when migration mode freeze is enabled.
pub fn migration_mode_frozen() -> bool {
    MIGRATION_FREEZE.load(Ordering::Relaxed)
}
