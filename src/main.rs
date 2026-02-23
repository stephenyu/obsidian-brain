mod config;
mod db;
mod embeddings;
mod index;
mod search;

use clap::Parser;
use crate::config::{AppPaths, Config, load_config, save_config};
use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use crate::index::{run_index, Meta};
use crate::search::run_search;
use anyhow::{Context, Result};
use std::path::PathBuf;
use chrono::{Utc, Duration};
use std::fs;

#[derive(Parser)]
#[command(name = "ob")]
#[command(about = "Obsidian Brain - Semantic search for your vault", long_about = None)]
struct Cli {
    /// Search query
    query: Option<String>,

    #[arg(short, long)]
    index: bool,

    #[arg(long)]
    init: Option<PathBuf>,

    #[arg(short, long)]
    force: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::from_env()?;

    // Handle --init
    if let Some(vault_path) = cli.init {
        let abs_path = fs::canonicalize(vault_path).context("Could not find vault path")?;
        let config = Config { vault_path: abs_path };
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
    } else if !cli.index && !cli.force {
        println!("Usage: ob <QUERY> or ob --index");
    }

    Ok(())
}
