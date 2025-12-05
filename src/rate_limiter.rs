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
                // Start with 1 token instead of max_tokens to prevent startup bursts
                // This allows immediate responsiveness (1 request) without hammering
                // the API with multiple requests when the daemon starts/restarts.
                // After the first request, proper rate limiting kicks in.
                tokens: 1,
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
    async fn test_rate_limiter_allows_single_initial_request() {
        // Rate limiter starts with 1 token to prevent startup bursts
        let limiter = RateLimiter::new(5, Duration::from_millis(100));

        // First request should be immediate (uses the initial token)
        let start = Instant::now();
        limiter.acquire().await;
        let first_duration = start.elapsed();

        // Should complete almost instantly (well under 50ms)
        assert!(
            first_duration < Duration::from_millis(50),
            "First request should be immediate, took {:?}",
            first_duration
        );

        // Second request should wait for refill (100ms)
        let start = Instant::now();
        limiter.acquire().await;
        let second_duration = start.elapsed();

        // Should take at least 100ms (the refill_rate)
        assert!(
            second_duration >= Duration::from_millis(100),
            "Second request should wait for refill, took {:?}",
            second_duration
        );

        // Should complete within reasonable time (100ms + tolerance)
        assert!(
            second_duration < Duration::from_millis(150),
            "Second request took too long: {:?}",
            second_duration
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_over_time() {
        let limiter = RateLimiter::new(5, Duration::from_millis(100));

        // Consume the initial token
        limiter.acquire().await;

        // Wait for refill (250ms should refill 2 tokens at 100ms each)
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Should have refilled at least 2 tokens, so these should not block
        limiter.acquire().await;
        limiter.acquire().await;
    }

    #[tokio::test]
    async fn test_rate_limiter_respects_requests_per_minute() {
        // Simulate the daemon's rate limiter configuration
        // Using 120 req/min for faster testing (500ms per request)
        let requests_per_minute = 120;
        let limiter = RateLimiter::new(
            requests_per_minute,
            Duration::from_secs(60) / requests_per_minute,
        );

        // First request uses the initial token (should be instant)
        let start = Instant::now();
        limiter.acquire().await;
        let first_duration = start.elapsed();

        assert!(
            first_duration < Duration::from_millis(50),
            "First request should be instant, took {:?}",
            first_duration
        );

        // Next 6 requests should be rate-limited
        // With 120 req/min, each token takes 500ms to refill
        // Test that 6 requests take approximately 3 seconds (6 * 500ms)
        let start = Instant::now();
        for _ in 0..6 {
            limiter.acquire().await;
        }
        let rate_limited_duration = start.elapsed();

        // Should take between 2.5s and 3.5s (allowing some tolerance)
        let expected = Duration::from_millis(3000);
        let min_expected = expected - Duration::from_millis(500);
        let max_expected = expected + Duration::from_millis(500);

        assert!(
            rate_limited_duration >= min_expected,
            "Rate limiting too fast: expected ~{:?}, got {:?}",
            expected,
            rate_limited_duration
        );
        assert!(
            rate_limited_duration <= max_expected,
            "Rate limiting too slow: expected ~{:?}, got {:?}",
            expected,
            rate_limited_duration
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_with_low_rate() {
        // Test with a very low rate similar to REDDIT_RATE_LIMIT_PER_MINUTE=4
        // Using 12 req/min for reasonable test duration (5 seconds per token)
        let requests_per_minute = 12;
        let limiter = RateLimiter::new(
            requests_per_minute,
            Duration::from_secs(60) / requests_per_minute,
        );

        // First request uses the initial token (should be instant)
        let start = Instant::now();
        limiter.acquire().await;
        let first_duration = start.elapsed();

        assert!(
            first_duration < Duration::from_millis(50),
            "First request should be instant, took {:?}",
            first_duration
        );

        // Next 2 requests should each take ~5 seconds to refill (total ~10s)
        let start = Instant::now();
        limiter.acquire().await;
        limiter.acquire().await;
        let duration = start.elapsed();

        // Should take between 9s and 11s
        assert!(
            duration >= Duration::from_secs(9),
            "Rate limiting too fast: expected ~10s, got {:?}",
            duration
        );
        assert!(
            duration <= Duration::from_secs(11),
            "Rate limiting too slow: expected ~10s, got {:?}",
            duration
        );
    }
}
