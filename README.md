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
REDDIT_RATE_LIMIT_PER_MINUTE=4
REDDIT_USER_AGENT=reddit_notifier/1.0 (by u/yourusername)

# Optional: Reddit Authentication (for higher rate limits and access to private subreddits)
# REDDIT_SESSION_COOKIE=your_reddit_session_cookie_value

# Optional: Database connection retry configuration
# DB_MAX_RETRIES=5              # Max connection attempts (default: 5)
# DB_INITIAL_DELAY_MS=500       # Initial retry delay in ms (default: 500)
# DB_MAX_DELAY_MS=5000          # Max retry delay in ms (default: 5000)
```

**Required Variables:**
- `DATABASE_URL` - SQLite database path
- `REDDIT_USER_AGENT` - User agent string (Reddit requires this to identify your app and username)

**Optional Variables:**
- `REDDIT_RATE_LIMIT_PER_MINUTE` - Number of Reddit polls per minute (default: 4)
- `REDDIT_SESSION_COOKIE` - Reddit session cookie for authenticated requests (enables higher rate limits)
- `DB_MAX_RETRIES` - Maximum database connection attempts at startup (default: 5)
- `DB_INITIAL_DELAY_MS` - Initial delay between retry attempts in milliseconds (default: 500)
- `DB_MAX_DELAY_MS` - Maximum delay between retry attempts in milliseconds (default: 5000)

**Reddit Authentication:**
Reddit authentication is optional but recommended for production use. Authenticated requests provide:
- Higher rate limits (up to 100 requests per minute vs ~60 for unauthenticated)
- Access to private subreddits you're subscribed to
- More reliable API access with fewer rate limit errors

**How to get your Reddit session cookie:**

1. **Log into Reddit** in your web browser
2. **Open browser developer tools** (F12 or right-click → Inspect)
3. **Go to the Network tab**
4. **Visit any Reddit page** (e.g., https://reddit.com/r/all)
5. **Find a request to reddit.com** in the Network tab
6. **Look for the Cookie header** in the request headers
7. **Copy the value after `reddit_session=`** (it will be a long string of letters and numbers)
8. **Add it to your .env file**: `REDDIT_SESSION_COOKIE=your_session_cookie_value`

**Alternative method using browser storage:**
1. **Log into Reddit** in your web browser
2. **Open browser developer tools** (F12)
3. **Go to Application/Storage tab**
4. **Navigate to Cookies → reddit.com**
5. **Find the `reddit_session` cookie**
6. **Copy its value**

⚠️ **Security Notes**: 
- Store your session cookie securely and never commit it to version control
- Session cookies expire - you'll need to refresh them periodically (typically every few months)
- Don't share your session cookie as it provides access to your Reddit account

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
=======
## 🏥 Docker Healthcheck

The container includes a built-in healthcheck that validates the database is functional:

- **Check Interval:** Every 30 seconds
- **Timeout:** 5 seconds
- **Start Period:** 10 seconds (grace period for initial setup)
- **Retries:** 3 failed checks before marking unhealthy

The healthcheck uses a dedicated Rust binary (~3.2MB stripped) that:
- ✅ Reads `DATABASE_URL` environment variable (same as the app)
- ✅ Connects to the SQLite database in read-only mode
- ✅ Executes `SELECT COUNT(*) FROM subscriptions` to verify schema exists
- ✅ Validates the exact database the application is using
- ✅ Works in scratch container without shell or additional utilities

**Monitor health status:**
```bash
docker ps                           # Check STATUS column
docker inspect --format='{{.State.Health.Status}}' <container_id>
docker inspect <container_id> | jq '.[0].State.Health'
```

**Health states:**
- `starting` - Container is starting up (within start-period)
- `healthy` - Database exists, is readable, and has valid schema
- `unhealthy` - Database missing, locked, corrupted, or missing schema

**Healthcheck validates:**
- Database file exists and is accessible
- SQLite can open the database
- Schema is initialized (subscriptions table exists)
- Database is not corrupted or locked

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
