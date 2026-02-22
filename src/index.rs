use std::path::Path;
use walkdir::WalkDir;
use crate::db::{Database, ChunkMeta};
use crate::embeddings::EmbeddingEngine;
use crate::config::Config;
use anyhow::Result;
use std::fs;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Meta {
    pub last_sync: DateTime<Utc>,
}

pub fn run_index(config: &Config, db: &mut Database, engine: &EmbeddingEngine, data_dir: &Path, force: bool) -> Result<()> {
    let meta_file = data_dir.join("meta.json");

    let last_sync = if !force && meta_file.exists() {
        let content = fs::read_to_string(&meta_file)?;
        let meta: Meta = serde_json::from_str(&content)?;
        Some(meta.last_sync)
    } else {
        None
    };

    println!("🚀 Starting Indexing...");

    let ignore_folders = [".obsidian", ".git", ".stfolder", "templates"];
    let mut files_processed = 0;

    for entry in WalkDir::new(&config.vault_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !ignore_folders.contains(&name.as_ref())
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() || !entry.path().extension().map_or(false, |ext| ext == "md") {
            continue;
        }

        let path = entry.path();
        let metadata = fs::metadata(path)?;
        let mtime: DateTime<Utc> = metadata.modified()?.into();

        if let Some(last) = last_sync {
            if mtime <= last {
                continue;
            }
        }

        process_file(path, &config.vault_path, db, engine, mtime.timestamp())?;
        files_processed += 1;
        if files_processed % 10 == 0 {
            println!("Processed {} files...", files_processed);
        }
    }

    db.save()?;

    let meta = Meta { last_sync: Utc::now() };
    fs::write(meta_file, serde_json::to_string(&meta)?)?;

    println!("✅ Indexed {} files.", files_processed);
    Ok(())
}

fn process_file(
    path: &Path,
    vault_root: &Path,
    db: &mut Database,
    engine: &EmbeddingEngine,
    mtime: i64,
) -> Result<()> {
    let rel_path = path.strip_prefix(vault_root)?.to_string_lossy().to_string();
    let filename = path.file_stem().unwrap_or_default().to_string_lossy().to_string();

    // Delete old entries for this file
    db.delete_by_path(&rel_path);

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(());
    }

    // Context injection
    let breadcrumb = path.parent()
        .and_then(|p| p.strip_prefix(vault_root).ok())
        .map(|p| p.to_string_lossy().replace("/", " > "))
        .unwrap_or_default();

    let identity_header = format!(
        "FILE_NAME: {}\nHOLDER_FOLDERS: {}\nDOCUMENT_SUBJECT: {}\n--- START OF CONTENT ---\n",
        filename, breadcrumb, filename
    );
    let full_text = identity_header + &content;

    // Chunk
    let chunks: Vec<String> = full_text.chars()
        .collect::<Vec<char>>()
        .chunks(1000)
        .map(|c| c.iter().collect())
        .collect();

    if chunks.is_empty() {
        return Ok(());
    }

    // Embed
    let embeddings = engine.embed(chunks.clone())?;

    // Build chunk metadata
    let metas: Vec<ChunkMeta> = chunks.into_iter().map(|text| ChunkMeta {
        id: 0, // assigned by db.insert_chunks
        path: rel_path.clone(),
        filename: filename.clone(),
        text,
        mtime,
    }).collect();

    db.insert_chunks(metas, embeddings)?;

    Ok(())
}
