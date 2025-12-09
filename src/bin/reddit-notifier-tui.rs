use anyhow::Result;
use dotenvy::dotenv;
use reddit_notifier::db_connection::{connect_with_retry, ConnectionConfig};
use reddit_notifier::models::config::AppConfig;
use reddit_notifier::services::SqliteDatabaseService;
use reddit_notifier::tui::App;
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Load configuration
    let cfg = AppConfig::from_env()?;

    // Connect to database with retry logic
    let connect_options = SqliteConnectOptions::from_str(&cfg.database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));

    // Configure pool for SQLite (low max_connections to reduce contention)
    let retry_config = ConnectionConfig::from_env();
    let pool = connect_with_retry(
        connect_options,
        3, // max_connections (lower for TUI)
        std::time::Duration::from_secs(300), // idle_timeout
        Some(retry_config),
    )
    .await?;

    // Run migrations
    sqlx::migrate!().run(&pool).await?;

    // Initialize terminal
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // Create database service and app
    let db = Arc::new(SqliteDatabaseService::new(pool));
    let mut app = App::new(db)?;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    ratatui::restore();

    result
}
