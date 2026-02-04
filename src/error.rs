use std::io;
use thiserror::Error;

/// Custom error types for the clean-rs application
#[derive(Error, Debug)]
pub enum CleanError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to delete file: {path}")]
    DeleteFailed { path: String },

    #[error("Platform not supported: {0}")]
    NotSupported(String),

    #[error("Windows API error: {0}")]
    WindowsError(String),
}

/// Result type alias for cleaner error handling
pub type Result<T> = std::result::Result<T, CleanError>;