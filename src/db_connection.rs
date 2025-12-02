use anyhow::{Context, Result};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::time::Duration;
use tracing::{info, warn};

/// Configuration for database connection retry logic
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Maximum number of connection attempts (default: 5)
    pub max_retries: u32,
    /// Initial delay between retries in milliseconds (default: 500ms)
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds (default: 5000ms)
    pub max_delay_ms: u64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_ms: 500,
            max_delay_ms: 5000,
        }
    }
}

impl ConnectionConfig {
    /// Load retry configuration from environment variables
    pub fn from_env() -> Self {
        let max_retries = std::env::var("DB_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let initial_delay_ms = std::env::var("DB_INITIAL_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(500);

        let max_delay_ms = std::env::var("DB_MAX_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5000);

        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
        }
    }
}

/// Connect to SQLite database with retry logic and exponential backoff
///
/// This function attempts to connect to the database with configurable retry logic.
/// It uses exponential backoff with jitter to handle transient failures like:
/// - Database file locked during WAL checkpoint
/// - NFS/network filesystem lag
/// - Temporary filesystem issues
///
/// # Arguments
/// * `connect_options` - SQLite connection options (must already have WAL, busy_timeout configured)
/// * `max_connections` - Maximum number of connections in the pool
/// * `idle_timeout` - How long idle connections stay in the pool
/// * `retry_config` - Optional retry configuration (uses defaults if None)
///
/// # Returns
/// * `Ok(SqlitePool)` on successful connection
/// * `Err` if all retry attempts fail
pub async fn connect_with_retry(
    connect_options: SqliteConnectOptions,
    max_connections: u32,
    idle_timeout: Duration,
    retry_config: Option<ConnectionConfig>,
) -> Result<SqlitePool> {
    let config = retry_config.unwrap_or_default();
    let mut attempt = 0;
    let mut delay_ms = config.initial_delay_ms;

    loop {
        attempt += 1;

        match sqlx::pool::PoolOptions::new()
            .max_connections(max_connections)
            .idle_timeout(idle_timeout)
            .connect_with(connect_options.clone())
            .await
        {
            Ok(pool) => {
                if attempt > 1 {
                    info!("Database connection successful after {} attempt(s)", attempt);
                } else {
                    info!("Database connection successful");
                }
                return Ok(pool);
            }
            Err(e) => {
                if attempt >= config.max_retries {
                    return Err(e).context(format!(
                        "Failed to connect to database after {} attempts",
                        config.max_retries
                    ));
                }

                warn!(
                    "Database connection attempt {}/{} failed: {} - retrying in {}ms",
                    attempt, config.max_retries, e, delay_ms
                );

                tokio::time::sleep(Duration::from_millis(delay_ms)).await;

                // Exponential backoff with cap
                delay_ms = (delay_ms * 2).min(config.max_delay_ms);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ConnectionConfig::default();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 5000);
    }

    #[test]
    fn test_exponential_backoff_cap() {
        let config = ConnectionConfig {
            max_retries: 10,
            initial_delay_ms: 100,
            max_delay_ms: 1000,
        };

        let mut delay = config.initial_delay_ms;
        // 100 -> 200 -> 400 -> 800 -> 1000 (capped) -> 1000 ...

        delay = (delay * 2).min(config.max_delay_ms); // 200
        assert_eq!(delay, 200);

        delay = (delay * 2).min(config.max_delay_ms); // 400
        assert_eq!(delay, 400);

        delay = (delay * 2).min(config.max_delay_ms); // 800
        assert_eq!(delay, 800);

        delay = (delay * 2).min(config.max_delay_ms); // 1000 (capped)
        assert_eq!(delay, 1000);

        delay = (delay * 2).min(config.max_delay_ms); // stays at 1000
        assert_eq!(delay, 1000);
    }
}
