use anyhow::{Context, Result};
use dotenvy::dotenv;
use reqwest::Client;
use sqlx::{sqlite::SqliteConnectOptions, Sqlite};
use sqlx::migrate::MigrateDatabase;
use std::str::FromStr;
use std::time::Duration;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use reddit_notifier::database::unique_subreddits;
use reddit_notifier::db_connection::{connect_with_retry, ConnectionConfig};
use reddit_notifier::models::AppConfig;
use reddit_notifier::poller::poll_combined_subreddits_loop;
use reddit_notifier::rate_limiter::RateLimiter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = AppConfig::from_env()?;

    // Create database file if it doesn't exist
    if !Sqlite::database_exists(&cfg.database_url).await? {
        Sqlite::create_database(&cfg.database_url).await?;
    }

    let connect_options = SqliteConnectOptions::from_str(&cfg.database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));

    // Configure pool for SQLite (low max_connections to reduce contention)
    let retry_config = ConnectionConfig::from_env();
    let pool = connect_with_retry(
        connect_options,
        5, // max_connections
        std::time::Duration::from_secs(300), // idle_timeout
        Some(retry_config),
    )
    .await
    .with_context(|| format!("failed to connect to {}", cfg.database_url))?;

    // Apply migrations at startup
    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Failed to run database migrations")?;

    let client = Client::builder()
        .user_agent(cfg.reddit_user_agent.clone())
        .build()?;

    let subreddits = unique_subreddits(&pool).await?;
    if subreddits.is_empty() {
        info!("No subscriptions configured. See README for setup SQL.");
        return Ok(());
    }

    // Create rate limiter for Reddit API calls
    // Rate limiter uses token bucket algorithm: refills one token per second
    // With rate_limit_per_minute tokens maximum
    // This controls how frequently we poll Reddit
    let rate_limiter = RateLimiter::new(
        cfg.rate_limit_per_minute,
        Duration::from_secs(1),
    );

    info!(
        "Starting combined poller for {} subreddit(s) with rate limiting ({} req/min)",
        subreddits.len(),
        cfg.rate_limit_per_minute
    );
    info!("Reddit notifier is running. Press Ctrl+C to shutdown gracefully.");

    // Use tokio::select! to race the poller against the shutdown signal
    // Whichever completes first will be handled
    tokio::select! {
        // Wait for Ctrl+C shutdown signal
        result = signal::ctrl_c() => {
            match result {
                Ok(()) => {
                    info!("Received shutdown signal, cleaning up...");
                }
                Err(err) => {
                    warn!("Unable to listen for shutdown signal: {}", err);
                }
            }
        }
        // Run the poller (this runs indefinitely until cancelled)
        result = poll_combined_subreddits_loop(pool, client, subreddits, rate_limiter) => {
            // The poller should run forever, so if it returns, something went wrong
            match result {
                Ok(()) => {
                    warn!("Poller completed unexpectedly");
                }
                Err(e) => {
                    warn!("Poller terminated with error: {}", e);
                }
            }
        }
    }

    info!("Shutdown complete");
    Ok(())
}
