use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
}

pub fn run_search(query: &str, db: &Database, engine: &EmbeddingEngine) -> Result<Vec<SearchResult>> {
    // Embed query
    let query_vector = engine.embed(vec![query.to_string()])?[0].clone();

    // Vector search
    let matches = db.search(&query_vector, 20)?;

    let mut file_map: HashMap<String, SearchResult> = HashMap::new();
    let query_words: Vec<String> = query.to_lowercase()
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
        if query_words.iter().any(|word| word.len() > 2 && filename.contains(word)) {
            score -= 0.7;
        }

        if !file_map.contains_key(&meta.path) || score < file_map[&meta.path].score {
            file_map.insert(meta.path.clone(), SearchResult {
                path: meta.path.clone(),
                score,
            });
        }
    }

    let mut sorted: Vec<SearchResult> = file_map.into_values().collect();
    sorted.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

    // Filter by confidence threshold
    let results = sorted.into_iter().filter(|r| r.score < 1.2).take(5).collect();

    Ok(results)
}
