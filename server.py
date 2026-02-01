from fastapi import FastAPI
import chromadb
import os

app = FastAPI()
client = chromadb.PersistentClient(path="./chroma_db")
collection = client.get_or_create_collection(name="obsidian_brain_v4")

@app.get("/search")
async def search(q: str):
    # Ask for 20 to ensure we don't miss short files buried by longer meeting notes
    results = collection.query(query_texts=[q], n_results=20)
    
    file_map = {}
    query_words = q.lower().split()

    for i in range(len(results['ids'][0])):
        full_path = results['metadatas'][0][i]['path']
        
        # --- PATH CLEANING ---
        # Returns 'Core/02-Area/People/Clayton G.md' instead of '/vault/Core/...'
        rel_path = os.path.relpath(full_path, "/vault")
        
        score = results['distances'][0][i]
        content = results['documents'][0][i]

        # RELEVANCE BOOST: If query words are in the filename, slash the score (better)
        filename_lower = os.path.basename(full_path).lower()
        if any(word in filename_lower for word in query_words if len(word) > 2):
            score -= 0.7 

        if rel_path not in file_map or score < file_map[rel_path]["score"]:
            # Snippet: Remove our injected header and take the first real sentence
            clean_content = content.split("---")[-1].strip()
            snippet = clean_content.split('\n')[0].split('. ')[0]
            if len(snippet) < 10 and len(clean_content) > 15:
                snippet = clean_content[:90].replace('\n', ' ') + "..."

            file_map[rel_path] = {
                "path": rel_path,
                "score": round(score, 3),
                "last_modified": results['metadatas'][0][i].get('last_modified', 'N/A'),
                "snippets": [snippet]
            }

    # Sort and return top 5
    sorted_results = sorted(file_map.values(), key=lambda x: x['score'])[:5]

    confident_results = [r for r in sorted_results if r['score'] < 1.2]

    return {
        "query": q,
        "count": len(confident_results),
        "results": confident_results
    }
