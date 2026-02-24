use crate::index::SyncManager;
use crate::search::{run_search, SearchResult};
use anyhow::{Context, Result};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

#[derive(Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

pub fn get_socket_path() -> String {
    if cfg!(windows) {
        r"\.\pipe\obra".to_string()
    } else {
        "/tmp/obra.sock".to_string()
    }
}

pub fn send_request(query: String) -> Result<Vec<SearchResult>> {
    let mut stream = LocalSocketStream::connect(get_socket_path())
        .context("Could not connect to daemon socket")?;

    let req = SearchRequest { query };
    let mut payload = serde_json::to_vec(&req)?;
    payload.push(b'\n');
    stream.write_all(&payload)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let resp: SearchResponse = serde_json::from_str(response_line.trim())?;
    Ok(resp.results)
}

pub fn start_server(manager: Arc<SyncManager>) -> Result<()> {
    let socket_path = get_socket_path();
    
    // Remove existing socket file on Unix
    if !cfg!(windows) {
        let _ = std::fs::remove_file(&socket_path);
    }

    let listener = LocalSocketListener::bind(socket_path)
        .context("Failed to bind local socket")?;

    println!("📡 IPC Server listening for search queries...");

    std::thread::spawn(move || {
        for stream in listener.incoming().filter_map(|s| s.ok()) {
            let manager = manager.clone();
            std::thread::spawn(move || {
                if let Err(e) = handle_client(stream, manager) {
                    eprintln!("❌ Error handling IPC client: {}", e);
                }
            });
        }
    });

    Ok(())
}

fn handle_client(stream: LocalSocketStream, manager: Arc<SyncManager>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let req: SearchRequest = serde_json::from_str(request_line.trim())?;

    let db = manager.db.lock().map_err(|_| anyhow::anyhow!("DB Lock failed"))?;
    let engine = &manager.engine;

    let results = run_search(&req.query, &db, engine)?;

    let resp = SearchResponse { results };
    let mut response_payload = serde_json::to_vec(&resp)?;
    response_payload.push(b'\n');

    let mut stream = reader.into_inner();
    stream.write_all(&response_payload)?;
    stream.flush()?;

    Ok(())
}
