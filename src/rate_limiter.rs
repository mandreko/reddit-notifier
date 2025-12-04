use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// A simple token bucket rate limiter
///
/// This rate limiter allows a certain number of operations per time window.
/// Reddit's API guidelines suggest staying under 60 requests per minute for
/// unauthenticated requests.
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
    max_tokens: u32,
    refill_rate: Duration,
}

struct RateLimiterState {
    tokens: u32,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `max_tokens` - Maximum number of tokens (requests) that can be stored
    /// * `refill_rate` - Time between adding new tokens
    ///
    /// # Example
    /// ```
    /// use reddit_notifier::rate_limiter::RateLimiter;
    /// use std::time::Duration;
    ///
    /// // Allow 60 requests per minute
    /// let limiter = RateLimiter::new(60, Duration::from_secs(1));
    /// ```
    pub fn new(max_tokens: u32, refill_rate: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: max_tokens,
                last_refill: Instant::now(),
            })),
            max_tokens,
            refill_rate,
        }
    }

    /// Wait until a token is available, then consume it
    ///
    /// This function will block (asynchronously) until a token becomes available.
    /// Once a token is available, it's consumed and the function returns.
    pub async fn acquire(&self) {
        loop {
            let mut state = self.state.lock().await;

            // Refill tokens based on elapsed time
            let now = Instant::now();
            let elapsed = now.duration_since(state.last_refill);
            let tokens_to_add = (elapsed.as_millis() / self.refill_rate.as_millis()) as u32;

            if tokens_to_add > 0 {
                state.tokens = (state.tokens + tokens_to_add).min(self.max_tokens);
                state.last_refill = now;
            }

            // If we have tokens, consume one and return
            if state.tokens > 0 {
                state.tokens -= 1;
                return;
            }

            // No tokens available, release the lock and wait before trying again
            drop(state);
            tokio::time::sleep(self.refill_rate / 2).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_initial_requests() {
        let limiter = RateLimiter::new(5, Duration::from_millis(100));

        // Should be able to make 5 requests immediately
        for _ in 0..5 {
            limiter.acquire().await;
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_over_time() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));

        // Consume all tokens
        limiter.acquire().await;
        limiter.acquire().await;

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Should have refilled at least 2 tokens
        limiter.acquire().await;
        limiter.acquire().await;
    }
}
