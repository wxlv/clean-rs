use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Represents a cleanup item that can be scanned and cleaned
#[derive(Debug, Clone)]
pub struct CleanupItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cleanup_type: CleanupType,
    pub enabled: bool,
}

/// Type of cleanup operation
#[derive(Debug, Clone)]
pub enum CleanupType {
    /// Clean a specific directory path
    Directory(PathBuf),
    /// Clean multiple directory patterns
    Directories(Vec<PathBuf>),
    /// Clean temp files in a directory
    TempFiles(PathBuf),
}

/// Result of scanning/cleaning a cleanup item
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub files: u64,
    pub directories: u64,
    pub size_bytes: u64,
    pub entries: u64, // For non-file items (like registry entries)
    pub has_data: bool,
}

impl CleanupResult {
    pub fn new() -> Self {
        Self {
            files: 0,
            directories: 0,
            size_bytes: 0,
            entries: 0,
            has_data: false,
        }
    }

    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn total_items(&self) -> u64 {
        self.files + self.directories + self.entries
    }
}

impl Default for CleanupResult {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupItem {
    /// Scan the cleanup item without deleting anything
    pub fn scan(&self) -> CleanupResult {
        match &self.cleanup_type {
            CleanupType::Directory(path) => self.scan_directory(path),
            CleanupType::Directories(paths) => {
                let mut result = CleanupResult::new();
                for path in paths {
                    let item_result = self.scan_directory(path);
                    result.files += item_result.files;
                    result.directories += item_result.directories;
                    result.size_bytes += item_result.size_bytes;
                    result.has_data = result.has_data || item_result.has_data;
                }
                result
            }
            CleanupType::TempFiles(path) => self.scan_temp_files(path),
        }
    }

    /// Clean the cleanup item (delete files)
    pub fn clean(&self) -> CleanupResult {
        match &self.cleanup_type {
            CleanupType::Directory(path) => self.clean_directory(path, false),
            CleanupType::Directories(paths) => {
                let mut result = CleanupResult::new();
                for path in paths {
                    let item_result = self.clean_directory(path, false);
                    result.files += item_result.files;
                    result.directories += item_result.directories;
                    result.size_bytes += item_result.size_bytes;
                    result.has_data = result.has_data || item_result.has_data;
                }
                result
            }
            CleanupType::TempFiles(path) => self.clean_temp_files(path, false),
        }
    }

    fn scan_directory(&self, path: &Path) -> CleanupResult {
        let mut result = CleanupResult::new();

        if !path.exists() {
            return result;
        }

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    if let Ok(metadata) = fs::metadata(&entry_path) {
                        result.files += 1;
                        result.size_bytes += metadata.len();
                        result.has_data = true;
                    }
                } else if entry_path.is_dir() {
                    let subdir_result = self.scan_directory(&entry_path);
                    result.files += subdir_result.files;
                    result.directories += 1 + subdir_result.directories;
                    result.size_bytes += subdir_result.size_bytes;
                    result.has_data = result.has_data || subdir_result.has_data;
                }
            }
        }

        debug!("Scanned {}: {} files, {} dirs, {:.2} MB", 
               self.name, result.files, result.directories, result.size_mb());
        result
    }

    fn clean_directory(&self, path: &Path, dry_run: bool) -> CleanupResult {
        let mut result = CleanupResult::new();

        if !path.exists() {
            return result;
        }

        // Scan first to get the result
        let scan_result = self.scan_directory(path);
        result.files = scan_result.files;
        result.directories = scan_result.directories;
        result.size_bytes = scan_result.size_bytes;
        result.has_data = scan_result.has_data;

        if dry_run {
            return result;
        }

        // Now actually clean
        info!("Cleaning {}...", self.name);
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    // Silent deletion - no error on failure
                    let _ = fs::remove_file(&entry_path);
                } else if entry_path.is_dir() {
                    // Silent deletion - no error on failure
                    let _ = fs::remove_dir_all(&entry_path);
                }
            }
        }

        result
    }

    fn scan_temp_files(&self, path: &Path) -> CleanupResult {
        let mut result = CleanupResult::new();

        if !path.exists() {
            return result;
        }

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                
                // Look for temp file patterns
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();
                
                let is_temp = name_str.contains(".tmp") 
                    || name_str.contains(".temp")
                    || name_str.starts_with("~")
                    || name_str.ends_with("~")
                    || name_str.contains("temp")
                    || name_str.contains("cache");
                
                if is_temp && entry_path.is_file() {
                    if let Ok(metadata) = fs::metadata(&entry_path) {
                        result.files += 1;
                        result.size_bytes += metadata.len();
                        result.has_data = true;
                    }
                }
                
                if entry_path.is_dir() {
                    let subdir_result = self.scan_temp_files(&entry_path);
                    result.files += subdir_result.files;
                    result.directories += subdir_result.directories;
                    result.size_bytes += subdir_result.size_bytes;
                    result.has_data = result.has_data || subdir_result.has_data;
                }
            }
        }

        debug!("Scanned temp files in {}: {} files, {:.2} MB", 
               self.name, result.files, result.size_mb());
        result
    }

    fn clean_temp_files(&self, path: &Path, dry_run: bool) -> CleanupResult {
        let mut result = CleanupResult::new();

        if !path.exists() {
            return result;
        }

        // Scan first
        let scan_result = self.scan_temp_files(path);
        result.files = scan_result.files;
        result.directories = scan_result.directories;
        result.size_bytes = scan_result.size_bytes;
        result.has_data = scan_result.has_data;

        if dry_run {
            return result;
        }

        // Now clean
        info!("Cleaning temp files in {}...", self.name);
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();
                
                let is_temp = name_str.contains(".tmp") 
                    || name_str.contains(".temp")
                    || name_str.starts_with("~")
                    || name_str.ends_with("~")
                    || name_str.contains("temp")
                    || name_str.contains("cache");
                
                if is_temp && entry_path.is_file() {
                    let _ = fs::remove_file(&entry_path);
                }
                
                if entry_path.is_dir() {
                    let _ = self.clean_temp_files(&entry_path, false);
                }
            }
        }

        result
    }
}

/// Get all available cleanup items for the current platform
pub fn get_all_cleanup_items() -> Vec<CleanupItem> {
    let mut items = Vec::new();

    let _home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let temp_dir = std::env::temp_dir();

    // 1. Temporary files directory
    items.push(CleanupItem {
        id: "temp_files".to_string(),
        name: "临时文件目录".to_string(),
        description: format!("系统临时文件目录: {}", temp_dir.display()),
        cleanup_type: CleanupType::Directory(temp_dir.clone()),
        enabled: true,
    });

    // 2. Windows Prefetch (Windows only)
    #[cfg(windows)]
    {
        let prefetch_dir = PathBuf::from("C:\\Windows\\Prefetch");
        items.push(CleanupItem {
            id: "prefetch".to_string(),
            name: "Windows Prefetch".to_string(),
            description: "Windows 预读文件缓存".to_string(),
            cleanup_type: CleanupType::Directory(prefetch_dir),
            enabled: true,
        });
    }

    // 3. Browser cache (Chrome)
    #[cfg(target_os = "windows")]
    if let Some(appdata) = dirs::data_local_dir() {
        let chrome_cache = appdata.join("Google\\Chrome\\User Data\\Default\\Cache");
        items.push(CleanupItem {
            id: "chrome_cache".to_string(),
            name: "Chrome 缓存".to_string(),
            description: "Chrome 浏览器缓存文件".to_string(),
            cleanup_type: CleanupType::Directory(chrome_cache),
            enabled: false,
        });
    }

    // 4. VS Code cache
    if let Some(appdata) = dirs::cache_dir() {
        let vscode_cache = appdata.join("Code");
        items.push(CleanupItem {
            id: "vscode_cache".to_string(),
            name: "VS Code 缓存".to_string(),
            description: "Visual Studio Code 缓存文件".to_string(),
            cleanup_type: CleanupType::Directory(vscode_cache),
            enabled: false,
        });
    }

    // 5. Package manager cache (cargo for Rust)
    #[cfg(target_os = "windows")]
    if let Some(home) = dirs::home_dir() {
        let cargo_cache = home.join(".cargo\\registry\\cache");
        items.push(CleanupItem {
            id: "cargo_cache".to_string(),
            name: "Cargo 缓存".to_string(),
            description: "Rust Cargo 包管理器缓存".to_string(),
            cleanup_type: CleanupType::Directory(cargo_cache),
            enabled: false,
        });
    }

    #[cfg(target_os = "windows")]
    if let Some(home) = dirs::home_dir() {
        let npm_cache = home.join("AppData\\Roaming\\npm-cache");
        items.push(CleanupItem {
            id: "npm_cache".to_string(),
            name: "NPM 缓存".to_string(),
            description: "Node.js NPM 包管理器缓存".to_string(),
            cleanup_type: CleanupType::Directory(npm_cache),
            enabled: false,
        });
    }

    // 6. Log files in temp directories
    items.push(CleanupItem {
        id: "log_files".to_string(),
        name: "日志文件".to_string(),
        description: "临时目录中的日志文件".to_string(),
        cleanup_type: CleanupType::TempFiles(temp_dir.clone()),
        enabled: true,
    });

    // 7. Thumbnail cache (Windows)
    #[cfg(windows)]
    if let Some(appdata) = dirs::data_local_dir() {
        let thumbnail_cache = appdata.join("Microsoft\\Windows\\Explorer");
        items.push(CleanupItem {
            id: "thumbnail_cache".to_string(),
            name: "缩略图缓存".to_string(),
            description: "Windows 文件缩略图缓存".to_string(),
            cleanup_type: CleanupType::Directory(thumbnail_cache),
            enabled: false,
        });
    }

    // 8. Recent documents (Windows)
    #[cfg(windows)]
    if let Some(appdata) = dirs::data_local_dir() {
        let recent_docs = appdata.join("Microsoft\\Windows\\Recent");
        items.push(CleanupItem {
            id: "recent_docs".to_string(),
            name: "最近文档".to_string(),
            description: "Windows 最近访问的文档列表".to_string(),
            cleanup_type: CleanupType::Directory(recent_docs),
            enabled: false,
        });
    }

    items
}