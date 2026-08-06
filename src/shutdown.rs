//! Graceful shutdown utilities for racing futures against shutdown signals
//! (Ctrl+C / SIGINT, and SIGTERM on Unix)

use anyhow::Result;
use tokio::signal;
use tracing::warn;

/// Result of racing a future against a shutdown signal
pub enum ShutdownRace<T> {
    /// Shutdown signal received (Ctrl+C / SIGTERM)
    Shutdown,
    /// The future completed with this result
    Completed(T),
}

/// Wait for a shutdown signal: Ctrl+C (SIGINT), plus SIGTERM on Unix.
///
/// SIGTERM matters because the daemon runs as PID 1 in a container, where
/// `docker stop` and orchestrator rollouts send SIGTERM - without a handler
/// the process ignores it and gets SIGKILLed after the grace period.
async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut term = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = signal::ctrl_c() => result.map_err(Into::into),
            _ = term.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        signal::ctrl_c().await.map_err(Into::into)
    }
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
        result = shutdown_signal() => {
            match result {
                Ok(()) => Ok(ShutdownRace::Shutdown),
                Err(err) => {
                    warn!("Unable to listen for shutdown signal: {}", err);
                    Err(err)
                }
            }
        }
        output = future => {
            Ok(ShutdownRace::Completed(output))
        }
    }
}
