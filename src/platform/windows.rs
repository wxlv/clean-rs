use crate::error::Result;
use std::ptr;
use tracing::{info, warn};
use winapi::um::shellapi::{
    SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
};

/// Clean the Windows Recycle Bin
pub fn clean_recycle_bin(dry_run: bool) -> Result<()> {
    info!("Checking Windows Recycle Bin...");
    
    unsafe {
        if dry_run {
            info!("[DRY RUN] Would empty the Recycle Bin");
            return Ok(());
        }
        
        // Empty the recycle bin
        info!("Emptying Recycle Bin...");
        let result = SHEmptyRecycleBinW(
            ptr::null_mut(),
            ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        );
        
        if result == 0 {
            info!("Recycle Bin emptied successfully");
            Ok(())
        } else {
            warn!("Failed to empty Recycle Bin (error: {}). This is not critical.", result);
            // Don't fail the entire operation if recycle bin fails
            Ok(())
        }
    }
}
