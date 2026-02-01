import chromadb
import os
import datetime

# Configuration
DB_PATH = os.path.abspath("./chroma_db")
VAULT_PATH = "/vault"

client = chromadb.PersistentClient(path=DB_PATH)
# Using a new version name to ensure we don't mix with old, "broken" data
collection = client.get_or_create_collection(name="obsidian_brain_v4")

def index_vault():
    print(f"🚀 Starting Context-Aware Indexing...")
    
    count = 0
    ignore_folders = {'.obsidian', '.git', '.stfolder', 'templates'}

    for root, dirs, files in os.walk(VAULT_PATH):
        # Skip internal system folders
        dirs[:] = [d for d in dirs if d not in ignore_folders]
        
        for file in files:
            if file.endswith(".md") and not file.startswith("."):
                file_path = os.path.join(root, file)
                filename = file.replace(".md", "")
                
                # Create a breadcrumb from the folder structure
                rel_path = os.path.relpath(file_path, VAULT_PATH)
                breadcrumb = " > ".join(rel_path.split(os.sep)[:-1])

                try:
                    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
                        content = f.read().strip()
                        if not content: continue

                        # --- CONTEXT INJECTION ---
                        # We repeat the filename and path at the top of every chunk
                        identity_header = (
                            f"FILE_NAME: {filename}\n"
                            f"HOLDER_FOLDERS: {breadcrumb}\n"
                            f"DOCUMENT_SUBJECT: {filename}\n"
                            f"--- START OF CONTENT ---\n"
                        )
                        full_text = identity_header + content

                        # Chunking (1000 chars)
                        chunks = [full_text[i:i+1000] for i in range(0, len(full_text), 1000)]
                        ids = [f"{file_path}_{i}" for i in range(len(chunks))]
                        
                        metadatas = [{
                            "path": file_path,
                            "last_modified": datetime.datetime.fromtimestamp(os.path.getmtime(file_path)).isoformat()
                        } for _ in range(len(chunks))]

                        collection.upsert(ids=ids, documents=chunks, metadatas=metadatas)
                        count += 1
                        if count % 100 == 0: print(f"Indexed {count} files...")

                except Exception as e:
                    print(f"❌ Error indexing {file}: {e}")

    print(f"✅ Success! {count} files contextualized. DB Total: {collection.count()}")

if __name__ == "__main__":
    index_vault()
