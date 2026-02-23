mod chunker;
mod config;
mod db;
mod embeddings;
mod index;
mod search;

use crate::config::{load_config, save_config, AppPaths, Config};
use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use crate::index::{run_index, Meta};
use crate::search::run_search;
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "obra")]
#[command(version)]
#[command(about = "Obsidian Brain - Semantic search for your vault", long_about = "A fast, local semantic search tool for your Obsidian vault. It uses local embeddings to find relevant notes even when exact keywords don't match.")]
#[command(arg_required_else_help(true))]
#[command(after_help = "EXAMPLES:\n    obra \"how to bake bread\"          # Search for notes\n    obra --index                      # Re-index the vault\n    obra --init ~/my-vault            # Initialize with a vault path")]
struct Cli {
    /// Search query to find relevant notes
    query: Option<String>,

    /// Re-index the vault to pick up changes
    #[arg(short, long)]
    index: bool,

    /// Initialize the tool with your Obsidian vault path
    #[arg(long, value_name = "VAULT_PATH")]
    init: Option<PathBuf>,

    /// Force a full re-indexing of all files (bypasses incremental sync)
    #[arg(short, long)]
    force: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::from_env()?;

    // Handle --init
    if let Some(vault_path) = cli.init {
        let abs_path = fs::canonicalize(vault_path).context("Could not find vault path")?;
        let config = Config {
            vault_path: abs_path,
        };
        save_config(&paths, &config)?;
        // Reset sync state so the next --index does a full scan
        let meta_file = paths.data_dir.join("meta.json");
        if meta_file.exists() {
            fs::remove_file(&meta_file)?;
        }
        println!("✅ Initialized with vault: {:?}", config.vault_path);
        return Ok(());
    }

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
