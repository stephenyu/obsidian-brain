mod chunker;
mod config;
mod db;
mod embeddings;
mod index;
mod ipc;
mod search;
mod watcher;

use crate::config::{load_config, save_config, AppPaths, Config};
use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use crate::index::{run_index, Meta, SyncManager};
use crate::ipc::{send_request, start_server};
use crate::search::run_search;
use crate::watcher::watch_vault;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    CustomMenuItem, SystemTray, SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
};

#[derive(Parser)]
#[command(name = "obra")]
#[command(version)]
#[command(about = "Obsidian Brain - Semantic search for your vault", long_about = "A fast, local semantic search tool for your Obsidian vault. It uses local embeddings to find relevant notes even when exact keywords don't match.")]
#[command(after_help = "EXAMPLES:\n    obra \"how to bake bread\"          # Search for notes\n    obra daemon                       # Start the background sync daemon\n    obra --index                      # Re-index the vault manually\n    obra init ~/my-vault              # Initialize with a vault path")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search query to find relevant notes
    query: Option<String>,

    /// Re-index the vault to pick up changes (manual sync)
    #[arg(short, long)]
    index: bool,

    /// Force a full re-indexing of all files (bypasses incremental sync)
    #[arg(short, long)]
    force: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the tool with your Obsidian vault path
    Init {
        #[arg(value_name = "VAULT_PATH")]
        vault_path: PathBuf,
    },
    /// Start the background daemon with a system tray icon
    Daemon {
        /// Run in the foreground instead of backgrounding
        #[arg(short, long)]
        foreground: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::from_env()?;

    // Handle Init
    if let Some(Commands::Init { vault_path }) = cli.command {
        let abs_path = fs::canonicalize(vault_path).context("Could not find vault path")?;
        let config = Config {
            vault_path: abs_path,
        };
        save_config(&paths, &config)?;
        let meta_file = paths.data_dir.join("meta.json");
        if meta_file.exists() {
            fs::remove_file(&meta_file)?;
        }
        println!("✅ Initialized with vault: {:?}", config.vault_path);
        return Ok(());
    }

    // Handle Daemon
    if let Some(Commands::Daemon { foreground }) = cli.command {
        return run_daemon(paths, foreground);
    }

    // Handle search - Try IPC first if daemon is running
    if let Some(ref query) = cli.query {
        if let Ok(results) = send_request(query.clone()) {
            let config = load_config(&paths)?;
            if results.is_empty() {
                eprintln!("No confident results found for '{}' (via daemon)", query);
            } else {
                for res in results {
                    println!("{}", config.vault_path.join(&res.path).display());
                }
            }
            return Ok(());
        }
    }

    // Legacy CLI behavior (Cold Start)
    let config = load_config(&paths)?;
    let mut db = Database::open(&paths.data_dir)?;
    let engine = EmbeddingEngine::new()?;

    // Handle --index or auto-sync
    let meta_file = paths.data_dir.join("meta.json");
    let needs_sync = if cli.index || cli.force {
        true
    } else if meta_file.exists() {
        let content = fs::read_to_string(&meta_file)?;
        let meta: Meta = serde_json::from_str(&content)?;
        Utc::now() - meta.last_sync > Duration::hours(24)
    } else {
        true
    };

    if needs_sync {
        if !cli.index && !cli.force {
            println!("🔔 Index is older than 24h, performing incremental sync...");
        }
        run_index(&config, &mut db, &engine, &paths.data_dir, cli.force)?;
    }

    // Handle search
    if let Some(query) = cli.query {
        let results = run_search(&query, &db, &engine)?;
        if results.is_empty() {
            eprintln!("No confident results found for '{}'", query);
        } else {
            for res in results {
                println!("{}", config.vault_path.join(&res.path).display());
            }
        }
    }

    Ok(())
}

fn run_daemon(paths: AppPaths, foreground: bool) -> Result<()> {
    use crate::ipc::get_socket_path;
    use interprocess::local_socket::LocalSocketStream;

    // 1. Check if already running
    if LocalSocketStream::connect(get_socket_path()).is_ok() {
        println!("ℹ️  Obra daemon is already running.");
        return Ok(());
    }

    // 2. Backgrounding logic
    if !foreground && std::env::var("OBRA_DAEMON_CHILD").is_err() {
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.log_file)
            .context("Failed to open log file")?;

        println!("🚀 Starting obra daemon in background...");
        
        std::process::Command::new(std::env::current_exe()?)
            .arg("daemon")
            .arg("--foreground")
            .env("OBRA_DAEMON_CHILD", "1")
            .stdout(std::process::Stdio::from(log_file.try_clone()?))
            .stderr(std::process::Stdio::from(log_file))
            .spawn()
            .context("Failed to spawn background process")?;
        
        // Give it a moment to start and check if it failed immediately
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        return Ok(());
    }

    let config = load_config(&paths)?;
    let db = Arc::new(Mutex::new(Database::open(&paths.data_dir)?));
    let engine = Arc::new(EmbeddingEngine::new()?);
    
    let manager = Arc::new(SyncManager::new(
        db.clone(),
        engine.clone(),
        config.vault_path.clone(),
        paths.data_dir.clone(),
    ));

    // System Tray Setup
    let quit = CustomMenuItem::new("quit".to_string(), "Exit Obra");
    let reindex = CustomMenuItem::new("reindex".to_string(), "Re-index All");
    let status = CustomMenuItem::new("status".to_string(), "Last indexed: Never").disabled();
    let tray_menu = SystemTrayMenu::new()
        .add_item(status)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(reindex)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(quit);

    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .setup({
            let manager = manager.clone();
            move |app| {
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);

                let tray_handle = app.tray_handle();
                manager.set_tray(tray_handle);

                // Start IPC Server
                start_server(manager.clone())?;
                
                // Start file watcher
                watch_vault(manager.clone())?;

                // Refresh tray status periodically
                let m = manager.clone();
                std::thread::spawn(move || loop {
                    m.refresh_tray_status();
                    std::thread::sleep(std::time::Duration::from_secs(60));
                });
                
                Ok(())
            }
        })
        .on_system_tray_event(move |app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    app.exit(0);
                }
                "reindex" => {
                    let m = manager.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = m.full_index(true) {
                            eprintln!("❌ Re-index failed: {}", e);
                        }
                    });
                }
                _ => {}
            },
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
