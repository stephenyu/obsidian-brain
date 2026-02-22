# Obsidian Brain (`ob`)

A fast, semantic search CLI for your Obsidian vault, built in Rust.

## Features
- **Semantic Search:** Uses local vector embeddings (BGE-Small) to find notes by meaning, not just keywords.
- **Incremental Indexing:** Automatically tracks file changes and only indexes what's necessary.
- **Zero Dependencies:** No Docker or Python required. Everything runs in a single binary.
- **Auto-Sync:** Automatically refreshes your index if it's older than 24 hours.

## Installation
```bash
# Clone the repository
git clone https://github.com/stephenyu/obsidian-brain
cd obsidian-brain

# Install via Cargo
cargo install --path .
```

## Usage
### 1. Initial Setup
Point the tool to your Obsidian vault:
```bash
ob --init /Users/yourname/Documents/Obsidian
```

### 2. Indexing
The tool indexes automatically on search, but you can force a sync:
```bash
ob --index
```

### 3. Searching
Search your vault just like `fzf`:
```bash
ob "stephens birthday"
```

## Data Locations
- **Config:** `~/.config/ob/config.json`
- **Database:** `~/.local/share/ob/lancedb`
- **Metadata:** `~/.local/share/ob/meta.json`
