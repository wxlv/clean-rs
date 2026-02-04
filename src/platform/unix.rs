use crate::error::{CleanError, Result};
use tracing::info;

/// Clean the Recycle Bin (not supported on Unix/Linux)
/// 
/// On Unix-like systems, there is no unified recycle bin. Each desktop
/// environment may have its own trash implementation.
pub fn clean_recycle_bin(dry_run: bool) -> Result<()> {
    if dry_run {
        info!("[DRY RUN] Recycle Bin cleaning is not supported on this platform");
    } else {
        info!("Recycle Bin cleaning is not supported on this platform");
    }
    Err(CleanError::NotSupported(
        "Recycle Bin is not available on Unix/Linux systems".to_string(),
    ))
}