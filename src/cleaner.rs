use crate::error::Result;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Calculate the total size of a directory recursively
pub fn get_dir_size(path: &Path) -> Result<u64> {
    let mut size = 0u64;
    
    if path.is_dir() {
        let entries = fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                size += get_dir_size(&path).unwrap_or(0);
            } else if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path) {
                    size += metadata.len();
                }
            }
        }
    } else if path.is_file() {
        if let Ok(metadata) = fs::metadata(path) {
            size = metadata.len();
        }
    }
    
    Ok(size)
}

/// Clean a directory by removing all files and subdirectories
pub fn clean_directory(path: &Path, dry_run: bool) -> Result<CleanResult> {
    info!("Cleaning directory: {}", path.display());
    
    if !path.exists() {
        warn!("Directory does not exist: {}", path.display());
        return Ok(CleanResult {
            files_deleted: 0,
            dirs_deleted: 0,
            bytes_cleaned: 0,
            errors: Vec::new(),
        });
    }

    let before_size = get_dir_size(path)?;
    let mut result = CleanResult {
        files_deleted: 0,
        dirs_deleted: 0,
        bytes_cleaned: 0,
        errors: Vec::new(),
    };

    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        
        if entry_path.is_file() {
            if dry_run {
                debug!("[DRY RUN] Would delete file: {}", entry_path.display());
                result.files_deleted += 1;
            } else {
                match fs::remove_file(&entry_path) {
                    Ok(()) => {
                        debug!("Deleted file: {}", entry_path.display());
                        result.files_deleted += 1;
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to delete file {}: {}", entry_path.display(), e);
                        error!("{}", err_msg);
                        result.errors.push(err_msg);
                    }
                }
            }
        } else if entry_path.is_dir() {
            if dry_run {
                debug!("[DRY RUN] Would delete directory: {}", entry_path.display());
                result.dirs_deleted += 1;
            } else {
                match fs::remove_dir_all(&entry_path) {
                    Ok(()) => {
                        debug!("Deleted directory: {}", entry_path.display());
                        result.dirs_deleted += 1;
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to delete directory {}: {}", entry_path.display(), e);
                        error!("{}", err_msg);
                        result.errors.push(err_msg);
                    }
                }
            }
        }
    }

    let after_size = get_dir_size(path)?;
    result.bytes_cleaned = before_size.saturating_sub(after_size);
    
    info!("Cleaned {} files, {} directories, {} bytes", 
          result.files_deleted, result.dirs_deleted, result.bytes_cleaned);
    
    Ok(result)
}

/// Result of a cleaning operation
#[derive(Debug, Clone)]
pub struct CleanResult {
    pub files_deleted: u64,
    pub dirs_deleted: u64,
    pub bytes_cleaned: u64,
    pub errors: Vec<String>,
}

impl CleanResult {
    pub fn is_empty(&self) -> bool {
        self.files_deleted == 0 && self.dirs_deleted == 0
    }
    
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    
    pub fn display_status(&self) -> String {
        let mut status = vec![
            format!("Files deleted: {}", self.files_deleted),
            format!("Directories deleted: {}", self.dirs_deleted),
            format!("Space freed: {:.2} MB", self.bytes_cleaned as f64 / (1024.0 * 1024.0)),
        ];
        
        if self.has_errors() {
            status.push(format!("Errors encountered: {}", self.errors.len()));
        }
        
        status.join("\n")
    }
}