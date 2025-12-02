# Reddit Notifier

Poll Reddit subreddits for new posts and send notifications to **Discord webhooks** and/or **Pushover** devices using a multi-threaded Rust application.

This app is designed to be:
- 🔧 Modular and extensible via traits
- 🚀 Asynchronous and multithreaded using `tokio`
- 💾 Backed by `sqlite` using `sqlx` for persistent state
- 🔐 Configurable via `.env` (using `dotenvy`)
- 🐛 Logged with structured tracing via `tracing`

---

## 📦 Features

- Polls `/r/<subreddit>/new.json` at a configured interval
- Deduplicates notifications across app restarts using a persistent database
- Supports multiple users with different notification preferences
- Sends notifications to:
    - ✅ Discord (via webhook)
    - ✅ Pushover (via API)
- Uses only one polling task per subreddit, even with many subscribers

---

## 🛠️ Setup Instructions

### 1. Clone & Build

```bash
git clone https://github.com/your-username/reddit-notifier.git
cd reddit-notifier
cargo build --release
```

### 2. Environment Configuration

Copy .env.example to .env and fill in the values:
```bash
DATABASE_URL=sqlite://data.db
POLL_INTERVAL_SECS=60
REDDIT_USER_AGENT=reddit_notifier/1.0 (by u/yourusername)

# Optional: Database connection retry configuration
# DB_MAX_RETRIES=5              # Max connection attempts (default: 5)
# DB_INITIAL_DELAY_MS=500       # Initial retry delay in ms (default: 500)
# DB_MAX_DELAY_MS=5000          # Max retry delay in ms (default: 5000)
```

**Required Variables:**
- `DATABASE_URL` - SQLite database path
- `REDDIT_USER_AGENT` - User agent string (Reddit requires this to identify your app and username)

**Optional Variables:**
- `POLL_INTERVAL_SECS` - Seconds between Reddit polls (default: 60)
- `DB_MAX_RETRIES` - Maximum database connection attempts at startup (default: 5)
- `DB_INITIAL_DELAY_MS` - Initial delay between retry attempts in milliseconds (default: 500)
- `DB_MAX_DELAY_MS` - Maximum delay between retry attempts in milliseconds (default: 5000)

**Connection Retry Behavior:**
The application uses exponential backoff when connecting to the database. This helps handle transient failures in Docker environments like:
- Database file locked during WAL checkpoint
- Network filesystem lag (though multi-writer scenarios are not supported - see warning below)
- Temporary file system issues

### 3.️ Database

Apply migrations

Install sqlx-cli (optional):
```bash
cargo install sqlx-cli
```

Then:
```bash
sqlx database create
sqlx migrate run
```
Or let the app apply them automatically at startup (enabled via sqlx::migrate!()).

# Example Setup SQL

```sql
-- Add a subreddit to monitor
INSERT INTO subscriptions (subreddit) VALUES ('deals');

-- Add a Discord endpoint
INSERT INTO endpoints (kind, config_json) VALUES (
  'discord',
  json('{
    "webhook_url": "https://discord.com/api/webhooks/XXX/YYY",
    "username": "RedditBot"
  }')
);

-- Add a Pushover endpoint
INSERT INTO endpoints (kind, config_json) VALUES (
  'pushover',
  json('{
    "token": "your_token",
    "user": "your_user_key"
  }')
);

-- Link subscription to endpoint
INSERT INTO subscription_endpoints (subscription_id, endpoint_id) VALUES (1, 1);
```

# 🐳 Docker

Build and Run Development Image

```bash
docker build -t reddit-notifier .
docker run --rm \
  -v $(pwd)/data:/app/data \
  --env-file .env \
  reddit-notifier
```

For production, you can use the pre-built image:
```bash
docker pull ghcr.io/mattandreko/reddit-notifier:latest
docker run --rm \
  -v $(pwd)/data:/data \
  ghcr.io/mandreko/reddit-notifier:latest
```

# 🐳 Docker-Compose Option

Create a docker-compose.yml file:
```bash
  services:
    reddit-notifier:
      image: ghcr.io/mandreko/reddit-notifier:latest
      volumes:
        - ./data:/data
      restart: unless-stopped

    reddit-notifier-tui:
      image: ghcr.io/mandreko/reddit-notifier:latest
      command: /app/reddit-notifier-tui
      volumes:
        - ./data:/data
      stdin_open: true
      tty: true
      profiles: ["tui"]
```

The main daemon notifier service will then run automatically, while making the tui front-end run only on-demand:

Example Usage:
```bash
  docker compose up -d                    # Run daemon
  docker compose run --rm reddit-notifier-tui  # Run TUI
```

---

## ⚠️ Important: Database Concurrency

**Do NOT run multiple instances of the daemon (`reddit-notifier`) sharing the same SQLite database file.**

While this application uses SQLite with WAL (Write-Ahead Logging) mode to support concurrent access between the daemon and TUI, **SQLite is not designed for multiple writer processes across different containers, hosts, or network volumes**.

### ✅ Supported Configurations:
- **One daemon** + **one or more TUI instances** sharing the same database (local filesystem or single-host volume)
- Running the daemon and TUI in separate containers on the **same host** sharing a local volume

### ❌ Unsupported Configurations:
- Multiple daemon instances writing to the same database file
- SQLite database on network-mounted storage (NFS, CIFS, cloud volumes) with multiple writers
- Running the daemon in a scaled Docker/Kubernetes deployment (replicas > 1)

### Why?
SQLite's file locking mechanisms don't work reliably across network filesystems or multiple processes on different hosts. This can lead to:
- Database corruption
- Lock timeout errors
- Data loss

### Recommendations:
- Run **exactly one daemon instance** per database file
- If you need to monitor multiple daemons, use separate database files and daemon instances
- The TUI can safely run concurrently with the daemon on the same host

---

# 📝 License

GPLv3 License © Matt Andreko

# 🙋 Contributing

Pull requests are welcome! Feel free to open issues or suggest improvements.
