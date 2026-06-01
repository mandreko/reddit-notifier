use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub rate_limit_per_minute: u32,
    pub reddit_user_agent: String,
    pub reddit_username: Option<String>,
    pub reddit_password: Option<String>,
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

        let rate_limit_per_minute = if requested_rate > MAX_RATE_LIMIT {
            tracing::warn!(
                "REDDIT_RATE_LIMIT_PER_MINUTE is set to {}, which exceeds the safe maximum of {}. Capping at {} req/min to avoid Reddit API bans.",
                requested_rate,
                MAX_RATE_LIMIT,
                MAX_RATE_LIMIT
            );
            MAX_RATE_LIMIT
        } else {
            requested_rate
        };

        let reddit_user_agent = std::env::var("REDDIT_USER_AGENT")
            .unwrap_or_else(|_| {
                format!(
                    "reddit_notifier/{} (https://github.com/mandreko/reddit-notifier)",
                    env!("CARGO_PKG_VERSION")
                )
            });

        let reddit_username = std::env::var("REDDIT_USERNAME").ok();
        let reddit_password = std::env::var("REDDIT_PASSWORD").ok();

        // Log authentication status (without revealing credentials)
        match (&reddit_username, &reddit_password) {
            (Some(_), Some(_)) => {
                tracing::info!("Reddit authentication configured - will use authenticated requests");
            },
            (Some(_), None) => {
                tracing::warn!("Reddit username provided but no password - using unauthenticated requests");
            },
            (None, Some(_)) => {
                tracing::warn!("Reddit password provided but no username - using unauthenticated requests");
            },
            (None, None) => {
                tracing::info!("No Reddit credentials provided - using unauthenticated requests");
            }
        }

        Ok(Self {
            database_url,
            rate_limit_per_minute,
            reddit_user_agent,
            reddit_username,
            reddit_password,
        })
    }
}
