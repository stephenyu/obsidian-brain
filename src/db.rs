use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

pub const VECTOR_DIM: usize = 384; // BGE-Small-EN-v1.5 dimension

#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkMeta {
    pub id: u64,
    pub path: String,
    pub filename: String,
    pub text: String,
    pub mtime: i64,
}

pub struct Database {
    pub index: Index,
    pub chunks: Vec<ChunkMeta>,
    data_dir: PathBuf,
    next_id: u64,
}

fn index_options() -> IndexOptions {
    IndexOptions {
        dimensions: VECTOR_DIM,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    }
}

impl Database {
    pub fn open(data_dir: &Path) -> Result<Self> {
        let index_path = data_dir.join("vectors.usearch");
        let chunks_path = data_dir.join("chunks.json");

        let index = Index::new(&index_options())?;
        if index_path.exists() {
            index.load(index_path.to_str().unwrap())?;
        }

        let chunks: Vec<ChunkMeta> = if chunks_path.exists() {
            let content = std::fs::read_to_string(&chunks_path)?;
            serde_json::from_str(&content)?
        } else {
            Vec::new()
        };

        let next_id = chunks.iter().map(|c| c.id + 1).max().unwrap_or(0);

        Ok(Self {
            index,
            chunks,
            data_dir: data_dir.to_path_buf(),
            next_id,
        })
    }

    pub fn save(&self) -> Result<()> {
        let index_path = self.data_dir.join("vectors.usearch");
        let chunks_path = self.data_dir.join("chunks.json");

        self.index.save(index_path.to_str().unwrap())?;
        let content = serde_json::to_string(&self.chunks)?;
        std::fs::write(&chunks_path, content)?;

        Ok(())
    }

    pub fn delete_by_path(&mut self, path: &str) {
        let to_remove: Vec<u64> = self.chunks.iter()
            .filter(|c| c.path == path)
            .map(|c| c.id)
            .collect();

        for id in &to_remove {
            let _ = self.index.remove(*id);
        }

        self.chunks.retain(|c| c.path != path);
    }

    pub fn insert_chunks(&mut self, mut metas: Vec<ChunkMeta>, vectors: Vec<Vec<f32>>) -> Result<()> {
        self.index.reserve(self.index.size() + vectors.len())?;

        for (meta, vec) in metas.iter_mut().zip(vectors.iter()) {
            meta.id = self.next_id;
            self.index.add(self.next_id, vec)?;
            self.next_id += 1;
        }

        self.chunks.extend(metas);
        Ok(())
    }

    pub fn search(&self, query_vec: &[f32], limit: usize) -> Result<Vec<(u64, f32)>> {
        let results = self.index.search(query_vec, limit)?;
        Ok(results.keys.into_iter().zip(results.distances).collect())
    }
}
