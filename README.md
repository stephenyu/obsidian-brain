# Obsidian Brain (`ob`)

A fast, semantic search CLI for your Obsidian vault, built in Rust.

`ob` uses local vector embeddings to find notes based on their meaning, allowing you to search your vault with natural language queries like "notes about machine learning" or "plans for the weekend".

## Features
- **Semantic Search:** Uses local vector embeddings (BGE-Small) to find notes by meaning, not just keywords.
- **Incremental Indexing:** Automatically tracks file changes and only indexes what's necessary.
- **Zero External Dependencies:** No Docker or Python required. Everything runs in a single, fast binary.
- **Auto-Sync:** Automatically refreshes your index if it's older than 24 hours.

## How it Works
1. **Scanning:** `ob` walks your Obsidian vault, ignoring folders like `.obsidian` and `.git`.
2. **Chunking:** Files are split into manageable chunks with overlapping context.
3. **Embedding:** Each chunk is converted into a 384-dimensional vector using the `BGE-Small-EN-v1.5` model via `candle`.
4. **Indexing:** Vectors are stored in a `usearch` index for ultra-fast similarity search.
5. **Search:** When you query, your query is also embedded and compared against the index. Results are ranked by cosine similarity and boosted by filename matches.

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
ob --init ~/Documents/MyVault
```

### 2. Searching
Search your vault using natural language:
```bash
ob "how to set up a rust project"
```

### 3. Indexing
Indexing happens automatically on search if needed, but you can force a sync:
```bash
ob --index
```

## Data Locations
- **Config:** `~/.config/ob/config.json`
- **Database:** `~/.local/share/ob/vectors.usearch`
- **Metadata:** `~/.local/share/ob/chunks.json`

## License
This project is licensed under the **Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0)** license.
- **Non-Commercial:** You may not use this material for commercial purposes.
- **Attribution:** You must give appropriate credit and indicate if changes were made.

See the [LICENSE](LICENSE) file for the full text.

## Contributing
Contributions are welcome! Please feel free to submit a Pull Request.
1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
