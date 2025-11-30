use anyhow::Result;
use dotenvy::dotenv;
use reddit_notifier::models::AppConfig;
use reddit_notifier::tui::App;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

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

    // Connect to database
    let connect_options = SqliteConnectOptions::from_str(&cfg.database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePool::connect_with(connect_options).await?;

    // Run migrations
    sqlx::migrate!().run(&pool).await?;

    // Initialize terminal
    let mut terminal = ratatui::init();
    terminal.clear()?;

    // Create and run app
    let mut app = App::new(pool)?;
    let result = app.run(&mut terminal).await;

    // Restore terminal
    ratatui::restore();

    result
}
