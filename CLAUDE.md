# CLAUDE.md

## Project Overview
Obsidian Brain (`obra`) is a semantic search CLI for Obsidian vaults using `usearch` and local embeddings via `candle`.

## Commands
```bash
# Initial setup
cargo run -- --init /path/to/vault

# Manual indexing
cargo run -- --index

# Search
cargo run -- "query string"

# Build for release
cargo build --release
```

## Architecture
- **src/main.rs**: CLI entry point and auto-sync logic.
- **src/index.rs**: Incremental indexing using file modification times.
- **src/search.rs**: Vector search with filename matching boost.
- **src/db.rs**: LanceDB schema and table management.
- **src/embeddings.rs**: Local embedding generation via `fastembed-rs`.
- **src/config.rs**: User configuration and data path management.
