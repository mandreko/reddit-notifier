use anyhow::{Context, Result};
use dotenvy::dotenv;
use reqwest::Client;
use sqlx::{SqlitePool, Sqlite};
use sqlx::migrate::MigrateDatabase;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use reddit_notifier::database::unique_subreddits;
use reddit_notifier::models::AppConfig;
use reddit_notifier::poller::poll_subreddit_loop;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = AppConfig::from_env()?;

    // Create database file if it doesn't exist
    if !Sqlite::database_exists(&cfg.database_url).await? {
        Sqlite::create_database(&cfg.database_url).await?;
    }

    let pool = SqlitePool::connect(&cfg.database_url)
        .await
        .with_context(|| format!("failed to connect to {}", cfg.database_url))?;

    // Apply migrations at startup
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let client = Client::builder()
        .user_agent(cfg.reddit_user_agent.clone())
        .build()?;

    let subreddits = unique_subreddits(&pool).await?;
    if subreddits.is_empty() {
        info!("No subscriptions configured. See README for setup SQL.");
        return Ok(());
    }

    info!("Starting pollers for {} subreddit(s)", subreddits.len());
    let mut handles = Vec::new();
    for sr in subreddits {
        let pool_clone = pool.clone();
        let client_clone = client.clone();
        let interval = cfg.poll_interval_secs;
        handles.push(tokio::spawn(async move {
            if let Err(e) = poll_subreddit_loop(pool_clone, client_clone, sr, interval).await {
                tracing::error!("Poll loop terminated with error: {}", e);
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    Ok(())
}
