use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub rate_limit_per_minute: u32,
    pub reddit_user_agent: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url =
            std::env::var("DATABASE_URL").context("DATABASE_URL is required (e.g., sqlite://data.db)")?;

        // Rate limit for Reddit API calls (requests per minute)
        // Default: 4 requests/minute (conservative to avoid Reddit's ~60/min limit)
        // Maximum: 45 requests/minute (safety cap to avoid Reddit bans)
        // Reddit's actual limit is ~60/min for unauthenticated requests
        const MAX_RATE_LIMIT: u32 = 45;
        const DEFAULT_RATE_LIMIT: u32 = 4;

        let requested_rate = std::env::var("REDDIT_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT);

        let rate_limit_per_minute = clamp_rate_limit(requested_rate, MAX_RATE_LIMIT);

        let reddit_user_agent = std::env::var("REDDIT_USER_AGENT")
            .unwrap_or_else(|_| {
                format!(
                    "reddit_notifier/{} (https://github.com/mandreko/reddit-notifier)",
                    env!("CARGO_PKG_VERSION")
                )
            });

        Ok(Self {
            database_url,
            rate_limit_per_minute,
            reddit_user_agent,
        })
    }
}

/// Clamp the requested rate limit to [1, max].
///
/// The lower bound matters: the rate limiter divides 60s by this value, so 0
/// would panic at startup with a divide-by-zero (and with `panic = "abort"`
/// plus a container restart policy, that becomes a crash loop).
fn clamp_rate_limit(requested: u32, max: u32) -> u32 {
    if requested > max {
        tracing::warn!(
            "REDDIT_RATE_LIMIT_PER_MINUTE is set to {}, which exceeds the safe maximum of {}. Capping at {} req/min to avoid Reddit API bans.",
            requested,
            max,
            max
        );
        max
    } else if requested == 0 {
        tracing::warn!(
            "REDDIT_RATE_LIMIT_PER_MINUTE is set to 0, which would disable polling entirely. Using 1 req/min instead."
        );
        1
    } else {
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_rate_limit_zero_becomes_one() {
        assert_eq!(clamp_rate_limit(0, 45), 1);
    }

    #[test]
    fn clamp_rate_limit_caps_at_max() {
        assert_eq!(clamp_rate_limit(100, 45), 45);
    }

    #[test]
    fn clamp_rate_limit_passes_through_valid_values() {
        assert_eq!(clamp_rate_limit(1, 45), 1);
        assert_eq!(clamp_rate_limit(4, 45), 4);
        assert_eq!(clamp_rate_limit(45, 45), 45);
    }
}
