# Obsidian Brain (`obra`)

A fast, semantic search CLI for your Obsidian vault, built in Rust.

`obra` uses local vector embeddings to find notes based on their meaning, allowing you to search your vault with natural language queries like "notes about machine learning" or "plans for the weekend".

## Features
- **Semantic Search:** Uses local vector embeddings (BGE-Small) to find notes by meaning, not just keywords.
- **Incremental Indexing:** Automatically tracks file changes and only indexes what's necessary.
- **Zero External Dependencies:** No Docker or Python required. Everything runs in a single, fast binary.
- **Daemon Mode:** Optional background process with real-time file watching and a system tray icon.

## How it Works
1. **Scanning:** `obra` walks your Obsidian vault, ignoring folders like `.obsidian` and `.git`.
2. **Chunking:** Files are split into manageable chunks with overlapping context.
3. **Embedding:** Each chunk is converted into a 384-dimensional vector using the `BGE-Small-EN-v1.5` model.
4. **Indexing:** Vectors are stored in a LanceDB index for fast similarity search.
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
obra init ~/Documents/MyVault
```

### 2. Searching
Search your vault using natural language:
```bash
obra "how to set up a rust project"
```

### 3. Indexing
Indexing happens automatically on search if needed, but you can force a sync:
```bash
obra --index          # Incremental sync
obra --force          # Full re-index of all files
```

---

## Two Modes of Operation

`obra` can be used in two ways: as a simple **CLI tool** (no daemon) or with a **background daemon** for always-fresh results.

### Mode 1: CLI (No Daemon)

Run `obra` directly when you need it. No background process required.

```bash
obra "my search query"
```

**How it works:** On each search, `obra` checks if the index is older than 24 hours. If it is, it performs an incremental sync before returning results. The embedding model is loaded fresh on every invocation.

**Pros:**
- Simple — no background process to manage.
- Zero resource usage when not searching.
- No system tray or GUI components needed.

**Cons:**
- **Cold start latency:** The first search (or one after a 24-hour gap) is slow because it must load the embedding model and re-index any changed files before returning results.
- Index may be slightly stale between syncs (up to 24 hours by default).
- Does not react to file changes in real time.

---

### Mode 2: Daemon

Run a persistent background process that watches your vault for changes and keeps the index up to date continuously.

```bash
# Start the daemon (backgrounds itself automatically)
obra daemon

# Or run in the foreground (useful for debugging)
obra daemon --foreground
```

The daemon starts a **system tray icon** (macOS menu bar) with options to re-index or quit. It also opens an IPC socket at `/tmp/obra.sock` that the `obra` CLI connects to automatically when present.

When the daemon is running, `obra "query"` sends the query over IPC to the daemon, which already has the embedding model loaded in memory and the index warm — returning results nearly instantly.

**Pros:**
- **Near-instant search:** No cold start. The model is already loaded and the index is always warm.
- **Real-time indexing:** Files are re-indexed automatically within seconds of being created, modified, or deleted.
- **System tray integration:** Status and controls available from the menu bar.

**Cons:**
- Consumes memory continuously (embedding model stays resident).
- Requires the daemon to be running; if it crashes, searches fall back to CLI mode automatically.
- System tray requires a display environment (not suitable for headless servers).

---

### Automatic Fallback

You don't need to think about which mode is active. When you run `obra "query"`:

1. `obra` first tries to connect to the daemon over IPC.
2. If the daemon is running, results are returned instantly via IPC.
3. If no daemon is found, `obra` falls back to the standard CLI cold-start path.

This means you can use the same `obra "query"` command regardless of whether the daemon is running.

---

## Data Locations
- **Config:** `~/.config/obra/config.json`
- **Database:** `~/.local/share/obra/`
- **Daemon Log:** `~/.local/share/obra/daemon.log`

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
