use anyhow::{Context, Result};
use dotenvy::dotenv;
use reqwest::Client;
use sqlx::{sqlite::SqliteConnectOptions, Sqlite};
use sqlx::migrate::MigrateDatabase;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use reddit_notifier::db_connection::{connect_with_retry, ConnectionConfig};
use reddit_notifier::models::config::AppConfig;
use reddit_notifier::poller::poll_loop;
use reddit_notifier::rate_limiter::RateLimiter;
use reddit_notifier::services::{DatabaseService, SqliteDatabaseService};
use reddit_notifier::shutdown::{race_with_shutdown, ShutdownRace};
use reddit_notifier::sources::build_post_source;

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

    // Create database service
    let db = Arc::new(SqliteDatabaseService::new(pool));

    let client = Client::builder()
        .user_agent(cfg.reddit_user_agent.clone())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?;

    let source = build_post_source(&cfg, client.clone());

    // Wait for subreddits to be configured
    // Check every 10 seconds until subscriptions exist in the database
    let subreddits = loop {
        let subs = db.unique_subreddits().await?;
        if !subs.is_empty() {
            break subs;
        }

        // Show errors to the console, so that the user knows that there's no data
        error!("No subscriptions configured in database. Waiting for configuration...");
        error!("Add subscriptions to the database to start monitoring. See README for setup SQL or use reddit-notifier-tui.");

        // Wait 10 seconds before checking again, or exit on Ctrl+C
        match race_with_shutdown(tokio::time::sleep(Duration::from_secs(10))).await? {
            ShutdownRace::Shutdown => {
                info!("Received shutdown signal during startup");
                return Ok(());
            }
            ShutdownRace::Completed(()) => {
                // Continue checking
            }
        }
    };

    // Token-bucket rate limiter for upstream requests.
    // Max tokens: per-source burst allowance (1 for Arctic Shift, = rate for Reddit JSON)
    // Refill rate: 60 seconds / rate_limit_per_minute (spreads requests evenly)
    let rate_limiter = RateLimiter::new(
        cfg.rate_limit_burst,
        Duration::from_secs(60) / cfg.rate_limit_per_minute,
    );

    info!(
        "Starting poller for {} subreddit(s) via {} ({} req/min, burst {}, min cycle interval {}s)",
        subreddits.len(),
        source.name(),
        cfg.rate_limit_per_minute,
        cfg.rate_limit_burst,
        cfg.poll_interval.as_secs()
    );
    info!("Reddit notifier is running. Press Ctrl+C to shutdown gracefully.");

    // Race the poller against the shutdown signal
    match race_with_shutdown(poll_loop(
        db,
        source,
        client,
        subreddits,
        rate_limiter,
        cfg.poll_interval,
    )).await? {
        ShutdownRace::Shutdown => {
            info!("Received shutdown signal, cleaning up...");
        }
        ShutdownRace::Completed(result) => {
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
