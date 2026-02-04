//! Clean-rs - System cleaning tool library
//!
//! This library provides functionality for cleaning system files and directories.

pub mod cleaner;
pub mod error;
pub mod platform;

pub use cleaner::{clean_directory, get_dir_size, CleanResult};
pub use error::{CleanError, Result};