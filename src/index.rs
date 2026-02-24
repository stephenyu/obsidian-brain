use crate::chunker::Chunker;
use crate::config::{Config, IGNORE_FOLDERS};
use crate::db::{ChunkMeta, Database};
use crate::embeddings::EmbeddingEngine;
use anyhow::Result;
use chrono::{DateTime, Utc, Local, Duration};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize)]
pub struct Meta {
    pub last_sync: DateTime<Utc>,
}

pub struct SyncManager {
    pub db: Arc<Mutex<Database>>,
    pub engine: Arc<EmbeddingEngine>,
    pub vault_path: PathBuf,
    pub data_dir: PathBuf,
    pub last_sync_time: Arc<Mutex<Option<DateTime<Utc>>>>,
    tray_handle: Mutex<Option<tauri::SystemTrayHandle>>,
}

impl SyncManager {
    pub fn new(
        db: Arc<Mutex<Database>>,
        engine: Arc<EmbeddingEngine>,
        vault_path: PathBuf,
        data_dir: PathBuf,
    ) -> Self {
        let meta_file = data_dir.join("meta.json");
        let last_sync = if meta_file.exists() {
            fs::read_to_string(&meta_file)
                .ok()
                .and_then(|content| serde_json::from_str::<Meta>(&content).ok())
                .map(|m| m.last_sync)
        } else {
            None
        };

        Self {
            db,
            engine,
            vault_path,
            data_dir,
            last_sync_time: Arc::new(Mutex::new(last_sync)),
            tray_handle: Mutex::new(None),
        }
    }

    pub fn set_tray(&self, handle: tauri::SystemTrayHandle) {
        {
            let mut h = self.tray_handle.lock().unwrap();
            *h = Some(handle);
        }
        self.refresh_tray_status();
    }

    fn update_status(&self) {
        let now = Utc::now();
        {
            let mut last = self.last_sync_time.lock().unwrap();
            *last = Some(now);
        }
        self.refresh_tray_status();
    }

    pub fn refresh_tray_status(&self) {
        let last_sync = {
            let last = self.last_sync_time.lock().unwrap();
            *last
        };

        let handle_lock = self.tray_handle.lock().unwrap();
        if let Some(ref handle) = *handle_lock {
            let status_text = if let Some(last_sync) = last_sync {
                let now = Utc::now();
                let duration = now.signed_duration_since(last_sync);
                let local_time: DateTime<Local> = DateTime::from(last_sync);
                format!(
                    "Last indexed: {} ({})",
                    local_time.format("%H:%M:%S"),
                    humanize_duration(duration)
                )
            } else {
                "Last indexed: Never".to_string()
            };
            let _ = handle.get_item("status").set_title(status_text);
        }
    }
}

fn humanize_duration(duration: Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = duration.num_days();
    format!("{}d ago", days)
}

impl SyncManager {
    pub fn full_index(&self, force: bool) -> Result<()> {
        let meta_file = self.data_dir.join("meta.json");

        let last_sync = if !force && meta_file.exists() {
            let content = fs::read_to_string(&meta_file)?;
            let meta: Meta = serde_json::from_str(&content)?;
            Some(meta.last_sync)
        } else {
            None
        };

        println!("🚀 Starting Indexing...");

        let mut paths_to_index = Vec::new();

        for entry in WalkDir::new(&self.vault_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !IGNORE_FOLDERS.contains(&name.as_ref())
            })
        {
            let entry = entry?;
            if !entry.file_type().is_file() || entry.path().extension() != Some(OsStr::new("md")) {
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
            paths_to_index.push((path.to_path_buf(), mtime.timestamp()));
        }

        if paths_to_index.is_empty() {
            println!("✅ No new files to index.");
            return Ok(());
        }

        println!("📂 Found {} files to index. Processing in batches...", paths_to_index.len());

        let mut db = self.db.lock().map_err(|_| anyhow::anyhow!("DB Lock failed"))?;
        
        let file_batch_size = 100;
        for (i, chunk) in paths_to_index.chunks(file_batch_size).enumerate() {
            println!("📦 Processing batch {}/{}...", i + 1, (paths_to_index.len() + file_batch_size - 1) / file_batch_size);
            process_batch(
                chunk,
                &self.vault_path,
                &mut db,
                &self.engine,
            )?;
        }

        db.save()?;
        drop(db);

        let meta = Meta {
            last_sync: Utc::now(),
        };
        fs::write(meta_file, serde_json::to_string(&meta)?)?;

        println!("✅ Indexed {} files.", paths_to_index.len());
        self.update_status();
        Ok(())
    }

    pub fn index_file(&self, path: &Path) -> Result<()> {
        let metadata = fs::metadata(path)?;
        let mtime: DateTime<Utc> = metadata.modified()?.into();
        
        let mut db = self.db.lock().map_err(|_| anyhow::anyhow!("Failed to lock database"))?;
        
        let paths = vec![(path.to_path_buf(), mtime.timestamp())];
        process_batch(&paths, &self.vault_path, &mut db, &self.engine)?;
        
        db.save()?;
        drop(db);
        self.update_status();
        Ok(())
    }
    
    pub fn remove_file(&self, path: &Path) -> Result<()> {
        let rel_path = path.strip_prefix(&self.vault_path)?.to_string_lossy().to_string();
        let mut db = self.db.lock().map_err(|_| anyhow::anyhow!("Failed to lock database"))?;
        db.delete_by_path(&rel_path);
        db.save()?;
        Ok(())
    }
}

pub fn run_index(
    config: &Config,
    db: &mut Database,
    engine: &EmbeddingEngine,
    data_dir: &Path,
    force: bool,
) -> Result<()> {
    let meta_file = data_dir.join("meta.json");

    let last_sync = if !force && meta_file.exists() {
        let content = fs::read_to_string(&meta_file)?;
        let meta: Meta = serde_json::from_str(&content)?;
        Some(meta.last_sync)
    } else {
        None
    };

    println!("🚀 Starting Indexing...");

    let mut paths_to_index = Vec::new();

    for entry in WalkDir::new(&config.vault_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !IGNORE_FOLDERS.contains(&name.as_ref())
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.path().extension() != Some(OsStr::new("md")) {
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
        paths_to_index.push((path.to_path_buf(), mtime.timestamp()));
    }

    if paths_to_index.is_empty() {
        println!("✅ No new files to index.");
        return Ok(());
    }

    println!("📂 Found {} files to index. Processing in batches...", paths_to_index.len());

    let file_batch_size = 100;
    for (i, chunk) in paths_to_index.chunks(file_batch_size).enumerate() {
        println!("📦 Processing batch {}/{}...", i + 1, (paths_to_index.len() + file_batch_size - 1) / file_batch_size);
        process_batch(chunk, &config.vault_path, db, engine)?;
    }

    db.save()?;

    let meta = Meta {
        last_sync: Utc::now(),
    };
    fs::write(meta_file, serde_json::to_string(&meta)?)?;

    println!("✅ Indexed {} files.", paths_to_index.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sync_manager_incremental() -> Result<()> {
        let vault_dir = tempdir()?;
        let data_dir = tempdir()?;
        
        let file_path = vault_dir.path().join("test.md");
        fs::write(&file_path, "# Hello\nThis is a test.")?;
        
        let db = Arc::new(Mutex::new(Database::open(data_dir.path())?));
        let engine = Arc::new(EmbeddingEngine::new()?);
        let manager = SyncManager::new(
            db.clone(),
            engine.clone(),
            vault_dir.path().to_path_buf(),
            data_dir.path().to_path_buf(),
        );
        
        // Initial index
        manager.index_file(&file_path)?;
        
        {
            let db_lock = db.lock().unwrap();
            assert_eq!(db_lock.chunks.len(), 1);
            assert_eq!(db_lock.chunks[0].filename, "test");
        }
        
        // Update file
        fs::write(&file_path, "# Hello Updated\nThis is an updated test.")?;
        manager.index_file(&file_path)?;
        
        {
            let db_lock = db.lock().unwrap();
            assert_eq!(db_lock.chunks.len(), 1);
            assert!(db_lock.chunks[0].text.contains("Updated"));
        }
        
        // Remove file
        manager.remove_file(&file_path)?;
        
        {
            let db_lock = db.lock().unwrap();
            assert_eq!(db_lock.chunks.len(), 0);
        }
        
        Ok(())
    }
}

pub fn process_batch(
    paths: &[(PathBuf, i64)],
    vault_root: &Path,
    db: &mut Database,
    engine: &EmbeddingEngine,
) -> Result<()> {
    // 1. Parallel Chunking
    let file_results: Vec<Result<(String, String, Vec<String>, i64)>> = paths
        .par_iter()
        .map(|(path, mtime)| {
            let rel_path = path.strip_prefix(vault_root)?.to_string_lossy().to_string();
            let filename = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let content = fs::read_to_string(path)?;
            if content.trim().is_empty() {
                return Ok((rel_path, filename, Vec::new(), *mtime));
            }

            // Context injection
            let breadcrumb = path
                .parent()
                .and_then(|p| p.strip_prefix(vault_root).ok())
                .map(|p| p.to_string_lossy().replace("/", " > "))
                .unwrap_or_default();

            let identity_header = format!(
                "FILE_NAME: {}\nHOLDER_FOLDERS: {}\nDOCUMENT_SUBJECT: {}\n--- START OF CONTENT ---\n",
                filename, breadcrumb, filename
            );
            let full_text = identity_header + &content;

            // Chunk
            let chunker = Chunker::default();
            let chunks = chunker.chunk(&full_text);

            Ok((rel_path, filename, chunks, *mtime))
        })
        .collect();

    // 2. Collect chunks and remove old entries
    let mut all_chunks = Vec::new();
    let mut chunk_metas = Vec::new();

    for res in file_results {
        let (rel_path, filename, chunks, mtime) = res?;
        
        // Delete old entries for this file
        db.delete_by_path(&rel_path);
        
        for text in chunks {
            all_chunks.push(text.clone());
            chunk_metas.push(ChunkMeta {
                id: 0, // assigned by db.insert_chunks
                path: rel_path.clone(),
                filename: filename.clone(),
                text,
                mtime,
            });
        }
    }

    if all_chunks.is_empty() {
        return Ok(());
    }

    // 3. Batched Embedding
    println!("🧠 Generating embeddings for {} chunks...", all_chunks.len());
    
    // We can process in smaller batches if needed, but the engine already batches.
    // However, BERT has a limit on sequence length and GPU/CPU memory.
    // Let's batch by 32 chunks at a time for safety and to show progress.
    let batch_size = 32;
    let mut all_embeddings = Vec::with_capacity(all_chunks.len());
    
    for i in (0..all_chunks.len()).step_by(batch_size) {
        let end = (i + batch_size).min(all_chunks.len());
        let batch = all_chunks[i..end].to_vec();
        let embeddings = engine.embed(batch)?;
        all_embeddings.extend(embeddings);
        
        if (i / batch_size) % 10 == 0 {
             println!("   ... {}/{}", end, all_chunks.len());
        }
    }

    // 4. Insert into DB
    db.insert_chunks(chunk_metas, all_embeddings)?;

    Ok(())
}
