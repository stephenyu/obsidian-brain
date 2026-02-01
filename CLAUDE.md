# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Obsidian Brain is a semantic search engine for Obsidian vaults using ChromaDB vector embeddings. It runs as a Docker container with a FastAPI server and scheduled indexing.

## Commands

```bash
# Build the Docker image
docker build -t brain-bookworm .

# Run the container (mounts local files and Obsidian vault)
docker run -d \
  --name obsidian-brain \
  -p 5001:5000 \
  -v "$(pwd)/server.py:/app/server.py" \
  -v "$(pwd)/indexer.py:/app/indexer.py" \
  -v "$(pwd)/chroma_db:/app/chroma_db" \
  -v "/Users/stephenyu/Documents/Obsidian:/vault:ro" \
  brain-bookworm

# Container management
docker stop obsidian-brain
docker restart obsidian-brain
docker ps  # Check if running

# Reset the database (if needed after schema changes)
rm -rf ./chroma_db

# Test search
curl "http://localhost:5001/search?q=movies"
```

## Architecture

**indexer.py** - Vault indexer that runs on cron (9 AM/9 PM):
- Walks `/vault` for `.md` files, skipping `.obsidian`, `.git`, `.stfolder`, `templates`
- Prepends context header (filename, folder breadcrumb) to improve search relevance
- Chunks documents at 1000 characters and upserts to ChromaDB collection `obsidian_brain_v4`

**server.py** - FastAPI search endpoint:
- `GET /search?q=<query>` returns top 5 results from 20 candidates
- Applies filename-match boost (subtracts 0.7 from distance score)
- Returns relative paths stripped of `/vault` prefix

**Data flow**: Obsidian vault → indexer chunks with context headers → ChromaDB → FastAPI queries → ranked results with snippets
