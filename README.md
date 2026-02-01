This application is run via Docker

First see if the container is running 

```
docker ps
```


When you make changes, stop the container First

```
docker stop obsidian-brain
```

You might need to remove the chroma db. 

```
rm -rf ./chroma_db
```


You can restart using

```
docker restart obsidian-brain
```

The whole run command is:
```
; docker run -d \
  --name obsidian-brain \
  -p 5001:5000 \
  -v "$(pwd)/server.py:/app/server.py" \
  -v "$(pwd)/indexer.py:/app/indexer.py" \
  -v "$(pwd)/chroma_db:/app/chroma_db" \
  -v "/Users/stephenyu/Documents/Obsidian:/vault:ro" \
  brain-bookworm
```


We query using curl on the host machine

```
curl "http://localhost:5001/search?q=movies"
```


To reindex you use 

```
docker exec obsidian-brain python indexer.py
```
