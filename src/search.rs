use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
}

pub fn run_search(
    query: &str,
    db: &Database,
    engine: &EmbeddingEngine,
) -> Result<Vec<SearchResult>> {
    // Embed query
    let query_vector = engine.embed(vec![query.to_string()])?[0].clone();

    // Vector search
    let matches = db.search(&query_vector, 20)?;

    let mut file_map: HashMap<String, SearchResult> = HashMap::new();
    let query_words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    for (key, distance) in matches {
        let Some(meta) = db.chunks.iter().find(|c| c.id == key) else {
            continue;
        };

        let filename = meta.filename.to_lowercase();
        let mut score = distance;

        // Filename boost
        if query_words
            .iter()
            .any(|word| word.len() > 2 && filename.contains(word))
        {
            score -= 0.7;
        }

        if !file_map.contains_key(&meta.path) || score < file_map[&meta.path].score {
            file_map.insert(
                meta.path.clone(),
                SearchResult {
                    path: meta.path.clone(),
                    score,
                },
            );
        }
    }

    let mut sorted: Vec<SearchResult> = file_map.into_values().collect();
    sorted.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

    // Filter by confidence threshold
    let results = sorted
        .into_iter()
        .filter(|r| r.score < 1.2)
        .take(5)
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ChunkMeta, Database, VECTOR_DIM};
    use tempfile::tempdir;

    #[test]
    fn test_search_ranking() -> Result<()> {
        let tmp = tempdir()?;
        let mut db = Database::open(tmp.path())?;

        let meta1 = ChunkMeta {
            id: 0,
            path: "apple.md".into(),
            filename: "apple".into(),
            text: "all about apples".into(),
            mtime: 0,
        };
        let vec1 = vec![0.1; VECTOR_DIM];
        db.insert_chunks(vec![meta1], vec![vec1.clone()])?;

        let matches = db.search(&vec1, 10)?;
        assert_eq!(matches.len(), 1);
        Ok(())
    }
}
