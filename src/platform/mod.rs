#[cfg(windows)]
pub mod windows;

#[cfg(not(windows))]
pub mod unix;

#[cfg(windows)]
pub use windows::clean_recycle_bin;

#[cfg(not(windows))]
pub use unix::clean_recycle_bin;