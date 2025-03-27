use std::env;
use std::fs;
use std::io;
use std::path::Path;

#[cfg(windows)]
use winapi::um::shellapi::{
    SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    SHQueryRecycleBinW, SHQUERYRBINFO,
};

fn get_dir_size(path: &Path) -> io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        size += get_dir_size(&path).unwrap_or(0);
                    } else {
                        size += fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
    } else if path.is_file() {
        size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    Ok(size)
}

fn clean_temp_dir() -> io::Result<u64> {
    let temp_dir = env::temp_dir();
    println!("Cleaning temp dir: {:?}", temp_dir);

    let before_size = get_dir_size(&temp_dir)?;
    let mut failed_files = Vec::new();

    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    match fs::remove_file(&path) {
                        Ok(()) => {}
                        Err(e) => failed_files.push((path, e)),
                    }
                } else if path.is_dir() {
                    match fs::remove_dir_all(&path) {
                        Ok(()) => {}
                        Err(e) => failed_files.push((path, e)),
                    }
                }
            }
        }
    }
    let after_size = get_dir_size(&temp_dir)?;
    let bytes_cleaned = before_size.saturating_sub(after_size);

    if !failed_files.is_empty() {
        eprintln!("Failed to delete some files:");
        for (path, e) in failed_files {
            eprintln!("{}: {}", path.display(), e);
        }
    }
    Ok(bytes_cleaned)
}

#[cfg(windows)]
fn clean_recycle_bin() -> io::Result<()> {
    unsafe {
        println!("Checking recycle bin status...");
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };
        let result = SHQueryRecycleBinW(std::ptr::null_mut(), &mut info);
        if result == 0 {
            println!("Recycle bin is empty");
            return Ok(());
        }
        let result = SHEmptyRecycleBinW(
            std::ptr::null_mut(),
            std::ptr::null(),
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        );
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(not(windows))]
fn clean_recycle_bin() -> io::Result<()> {
    println!("Not implemented for this platform");
    Ok(())
}

fn main() -> io::Result<()> {
    println!("Cleaning up...");

    match clean_temp_dir() {
        Ok(size) => println!(
            "Cleaned up {} MB from temp dir",
            size as f64 / (1024.0 * 1024.0)
        ),
        Err(e) => eprintln!("Error cleaning up temp dir: {}", e),
    }

    match clean_recycle_bin() {
        Ok(()) => println!("Cleaned up recycle bin"),
        Err(e) => eprintln!("Error cleaning up recycle bin: {}", e),
    }

    println!("Done");
    Ok(())
}
