use clean_rs::{clean_directory, get_dir_size, CleanResult};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_get_dir_size() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create test files
    fs::write(dir_path.join("file1.txt"), b"Hello, World!").unwrap();
    fs::write(dir_path.join("file2.txt"), vec![0u8; 1024]).unwrap();

    // Create subdirectory with file
    let subdir = dir_path.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file3.txt"), vec![0u8; 2048]).unwrap();

    let size = get_dir_size(dir_path).unwrap();
    
    // 13 + 1024 + 2048 = 3085 bytes
    assert_eq!(size, 3085);
}

#[test]
fn test_clean_directory_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create test structure
    fs::write(dir_path.join("file1.txt"), b"Hello, World!").unwrap();
    fs::write(dir_path.join("file2.txt"), vec![0u8; 1024]).unwrap();
    
    let subdir = dir_path.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file3.txt"), vec![0u8; 2048]).unwrap();

    // Dry run should not delete files
    let result = clean_directory(dir_path, true).unwrap();

    assert!(result.files_deleted > 0);
    assert!(result.dirs_deleted > 0);
    assert_eq!(result.bytes_cleaned, 3085);

    // Verify files still exist
    assert!(dir_path.join("file1.txt").exists());
    assert!(dir_path.join("file2.txt").exists());
    assert!(dir_path.join("subdir/file3.txt").exists());
}

#[test]
fn test_clean_directory_real() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create test structure
    fs::write(dir_path.join("file1.txt"), b"Hello, World!").unwrap();
    fs::write(dir_path.join("file2.txt"), vec![0u8; 1024]).unwrap();
    
    let subdir = dir_path.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file3.txt"), vec![0u8; 2048]).unwrap();

    // Real clean should delete files
    let result = clean_directory(dir_path, false).unwrap();

    assert!(result.files_deleted > 0);
    assert!(result.dirs_deleted >= 1);
    assert!(result.bytes_cleaned > 0);

    // Verify files are deleted
    assert!(!dir_path.join("file1.txt").exists());
    assert!(!dir_path.join("file2.txt").exists());
    assert!(!dir_path.join("subdir").exists());
}

#[test]
fn test_clean_result_methods() {
    let result = CleanResult {
        files_deleted: 10,
        dirs_deleted: 2,
        bytes_cleaned: 1024,
        errors: vec!["Error1".to_string()],
    };

    assert!(!result.is_empty());
    assert!(result.has_errors());

    let empty_result = CleanResult {
        files_deleted: 0,
        dirs_deleted: 0,
        bytes_cleaned: 0,
        errors: vec![],
    };

    assert!(empty_result.is_empty());
    assert!(!empty_result.has_errors());
}

#[test]
fn test_clean_result_display_status() {
    let result = CleanResult {
        files_deleted: 10,
        dirs_deleted: 2,
        bytes_cleaned: 1024000,
        errors: vec!["Error1".to_string(), "Error2".to_string()],
    };

    let status = result.display_status();
    assert!(status.contains("Files deleted: 10"));
    assert!(status.contains("Directories deleted: 2"));
    assert!(status.contains("0.98")); // Should be approximately 0.98 MB
    assert!(status.contains("Errors encountered: 2"));
}