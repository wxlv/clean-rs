mod cleanup_items;
mod error;
mod platform;
mod tui;

use anyhow::Result;
use clap::Parser;
use cleanup_items::CleanupType;
use error::CleanError;
use platform::clean_recycle_bin;
use std::env;
use std::path::PathBuf;
use tracing::{error, info, Level};

/// Clean Tools of Rust - A system cleaning tool with TUI interface
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Use TUI (Terminal User Interface) mode
    #[arg(short, long)]
    tui: bool,

    /// Clean temporary files only
    #[arg(short, long)]
    temp: bool,

    /// Clean recycle bin only (Windows)
    #[arg(short, long)]
    recycle: bool,

    /// Custom directory to clean
    #[arg(short = 'd', long, value_name = "DIR")]
    directory: Option<PathBuf>,

    /// Dry run - show what would be deleted without actually deleting
    #[arg(long)]
    dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Quiet mode - suppress output except errors
    #[arg(short, long)]
    quiet: bool,

    /// Pause before exit (useful when running from .exe on Windows)
    #[arg(long)]
    pause: bool,
}

impl Cli {
    /// Get log level based on flags
    fn log_level(&self) -> Level {
        if self.verbose {
            Level::DEBUG
        } else if self.quiet {
            Level::ERROR
        } else {
            Level::INFO
        }
    }
}

/// Initialize logging system (silent for TUI to avoid interfering with output)
fn init_logging(level: Level, silent: bool) {
    if silent {
        // Don't initialize logging for TUI mode
        return;
    }
    
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(level)
        .finish();

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Warning: Failed to set tracing subscriber: {}", e);
    }
}

/// Prevent console window from closing immediately on Windows
fn pause_if_needed(should_pause: bool) {
    if should_pause {
        println!("\n按回车键退出...");
        let mut input = String::new();
        let _ = std::io::stdin().read_line(&mut input);
    }
}

/// Clean with new cleanup_items module
fn clean_with_items(items: Vec<cleanup_items::CleanupItem>, dry_run: bool) -> (u64, u64) {
    let mut total_bytes = 0u64;
    let mut total_files = 0u64;

    for item in items {
        if !item.enabled {
            continue;
        }

        info!("Cleaning: {}", item.name);
        let result = item.scan();
        
        if result.has_data {
            if dry_run {
                println!("  [DRY RUN] Would clean: {} files ({:.2} MB)", 
                        result.files, result.size_mb());
            } else {
                println!("  Cleaning: {} files ({:.2} MB)", 
                        result.files, result.size_mb());
                // Actually clean
                let _ = item.clean();
            }
            total_bytes += result.size_bytes;
            total_files += result.files;
        }
    }

    (total_bytes, total_files)
}

/// Legacy: Clean temporary directory
fn clean_temp(dry_run: bool) -> Result<u64> {
    let temp_dir = env::temp_dir();
    info!("Cleaning temporary directory: {:?}", temp_dir);

    let item = cleanup_items::CleanupItem {
        id: "legacy_temp".to_string(),
        name: "临时文件目录".to_string(),
        description: "".to_string(),
        cleanup_type: CleanupType::Directory(temp_dir),
        enabled: true,
    };

    let result = item.scan();
    if !dry_run && result.has_data {
        let _ = item.clean();
    }

    Ok(result.size_bytes)
}

/// Legacy: Clean custom directory
fn clean_custom_directory(path: PathBuf, dry_run: bool) -> Result<u64> {
    info!("Cleaning custom directory: {:?}", path);

    let item = cleanup_items::CleanupItem {
        id: "legacy_custom".to_string(),
        name: "自定义目录".to_string(),
        description: path.display().to_string(),
        cleanup_type: CleanupType::Directory(path),
        enabled: true,
    };

    let result = item.scan();
    if !dry_run && result.has_data {
        let _ = item.clean();
    }

    Ok(result.size_bytes)
}

/// Display cleanup summary
fn display_summary(total_bytes: u64, dry_run: bool) {
    if dry_run {
        println!("\n[DRY RUN] Summary:");
        println!("Would free approximately {:.2} MB", total_bytes as f64 / (1024.0 * 1024.0));
    } else {
        println!("\nSummary:");
        println!("Freed {:.2} MB of disk space", total_bytes as f64 / (1024.0 * 1024.0));
    }
}

fn run_cli_mode(cli: &Cli) -> Result<()> {
    let mut total_bytes = 0u64;
    let mut has_error = false;
    let directory_provided = cli.directory.is_some();

    // Clean temporary files
    if cli.temp {
        match clean_temp(cli.dry_run) {
            Ok(bytes) => total_bytes += bytes,
            Err(e) => {
                error!("Failed to clean temporary directory: {}", e);
                has_error = true;
            }
        }
    }

    // Clean custom directory
    if let Some(dir) = &cli.directory {
        match clean_custom_directory(dir.clone(), cli.dry_run) {
            Ok(bytes) => total_bytes += bytes,
            Err(e) => {
                error!("Failed to clean custom directory: {}", e);
                has_error = true;
            }
        }
    }

    // Clean recycle bin
    if cli.recycle {
        if let Err(e) = clean_recycle_bin(cli.dry_run) {
            if let CleanError::NotSupported(_) = e {
                info!("{}", e);
            } else {
                error!("Failed to clean recycle bin: {}", e);
            }
        }
    }

    // If no specific options provided, clean everything
    if !cli.temp && !cli.recycle && !directory_provided {
        match clean_temp(cli.dry_run) {
            Ok(bytes) => total_bytes += bytes,
            Err(e) => {
                error!("Failed to clean temporary directory: {}", e);
                has_error = true;
            }
        }

        if let Err(e) = clean_recycle_bin(cli.dry_run) {
            if let CleanError::NotSupported(_) = e {
                info!("{}", e);
            } else {
                error!("Failed to clean recycle bin: {}", e);
            }
        }
    }

    // Display summary
    display_summary(total_bytes, cli.dry_run);

    if has_error {
        std::process::exit(1);
    }

    info!("Complete!");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Check if running without arguments (e.g., double-clicked .exe)
    // If no specific options provided, default to TUI mode
    let args: Vec<String> = std::env::args().collect();
    let has_args = args.len() > 1;
    let has_cli_options = cli.temp || cli.recycle || cli.directory.is_some();
    
    // Auto-detect TUI mode:
    // - If --tui flag is explicitly set, use it
    // - If no arguments at all OR just the program name, default to TUI
    // - If CLI-specific options are used, use CLI mode
    let use_tui = cli.tui || (!has_args && !has_cli_options);

    // Initialize logging (silent for TUI)
    init_logging(cli.log_level(), use_tui);

    // Check if TUI mode is requested
    if use_tui {
        info!("Starting TUI mode...");
        // Run TUI - no logging output to avoid interference
        let result = tui::run_tui();
        
        // Pause before exit if requested
        pause_if_needed(cli.pause);
        
        result?;
        Ok(())
    } else {
        // CLI mode
        info!("Clean-rs v{} starting", env!("CARGO_PKG_VERSION"));
        if cli.dry_run {
            info!("Running in DRY RUN mode - no files will be deleted");
        }

        run_cli_mode(&cli)?;
        
        // Pause before exit if requested (prevents console flash)
        pause_if_needed(cli.pause);
        
        Ok(())
    }
}
