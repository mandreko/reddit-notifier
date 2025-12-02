use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection};
use std::process;
use std::str::FromStr;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Get DATABASE_URL from environment - must match what the app uses
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Healthcheck failed: DATABASE_URL environment variable not set");
            process::exit(1);
        }
    };

    // Strip sqlite:// prefix to get file path
    let db_path = database_url.trim_start_matches("sqlite://");

    // Try to connect and query the database
    let result = check_database(db_path).await;

    match result {
        Ok(_) => {
            // Database is healthy
            process::exit(0);
        }
        Err(e) => {
            eprintln!("Healthcheck failed: {}", e);
            process::exit(1);
        }
    }
}

async fn check_database(db_path: &str) -> Result<(), String> {
    // Build connection options with short timeout
    let connect_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path))
        .map_err(|e| format!("Invalid database path: {}", e))?
        .read_only(true) // Healthcheck should not modify data
        .busy_timeout(std::time::Duration::from_secs(2));

    // Try to connect
    let mut conn = SqliteConnection::connect_with(&connect_options)
        .await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    // Perform a simple query to verify schema exists and database is readable
    let count: Result<i64, _> =
        sqlx::query_scalar("SELECT COUNT(*) FROM subscriptions")
            .fetch_one(&mut conn)
            .await;

    match count {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to query database: {}", e)),
    }
}
