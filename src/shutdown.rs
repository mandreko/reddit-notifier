//! Graceful shutdown utilities for racing futures against Ctrl+C signals

use anyhow::Result;
use tokio::signal;
use tracing::warn;

/// Result of racing a future against a shutdown signal
pub enum ShutdownRace<T> {
    /// Shutdown signal received (Ctrl+C)
    Shutdown,
    /// The future completed with this result
    Completed(T),
}

/// Race a future against Ctrl+C shutdown signal
///
/// Returns `ShutdownRace::Shutdown` if Ctrl+C was pressed, or
/// `ShutdownRace::Completed(T)` with the future's result.
///
/// # Example
/// ```no_run
/// use reddit_notifier::shutdown::{race_with_shutdown, ShutdownRace};
/// use tokio::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// match race_with_shutdown(tokio::time::sleep(Duration::from_secs(10))).await? {
///     ShutdownRace::Shutdown => {
///         println!("Shutdown requested");
///     }
///     ShutdownRace::Completed(()) => {
///         println!("Task completed");
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub async fn race_with_shutdown<F, T>(future: F) -> Result<ShutdownRace<T>>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        result = signal::ctrl_c() => {
            match result {
                Ok(()) => Ok(ShutdownRace::Shutdown),
                Err(err) => {
                    warn!("Unable to listen for shutdown signal: {}", err);
                    Err(err.into())
                }
            }
        }
        output = future => {
            Ok(ShutdownRace::Completed(output))
        }
    }
}
