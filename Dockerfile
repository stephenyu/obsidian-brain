FROM python:3.11-bookworm

# 1. Install system utilities
RUN apt-get update && apt-get install -y \
    cron \
    curl \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 2. Install Python packages (Removed pysqlite3-binary)
RUN pip install --no-cache-dir \
    chromadb \
    fastapi \
    uvicorn

# 3. Copy your logic
COPY indexer.py server.py .

# 4. Create the cron job (9 AM and 9 PM)
RUN echo "0 9,21 * * * /usr/local/bin/python /app/indexer.py /vault >> /var/log/cron.log 2>&1" > /etc/cron.d/obsidian-cron && \
    chmod 0644 /etc/cron.d/obsidian-cron && \
    crontab /etc/cron.d/obsidian-cron

# 5. Startup
CMD ["sh", "-c", "cron && uvicorn server:app --host 0.0.0.0 --port 5000"]
