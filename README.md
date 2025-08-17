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
```
💡 Reddit requires a User-Agent that clearly identifies your app and your Reddit username.


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

# 📝 License

GPLv3 License © Matt Andreko

# 🙋 Contributing

Pull requests are welcome! Feel free to open issues or suggest improvements.

