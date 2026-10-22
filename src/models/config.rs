use anyhow::{anyhow, Context, Result};
use std::str::FromStr;
use std::time::Duration;

/// Which backend the poller uses to discover new posts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PostSourceKind {
    /// Arctic Shift archive API (https://arctic-shift.photon-reddit.com). Default.
    #[default]
    ArcticShift,
    /// Reddit's unauthenticated `/r/<subs>/new.json` listing. Deprecated by Reddit;
    /// already blocked from many IPs and scheduled to end in 2027.
    RedditJson,
}

impl PostSourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostSourceKind::ArcticShift => "arctic_shift",
            PostSourceKind::RedditJson => "reddit_json",
        }
    }
}

impl FromStr for PostSourceKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "arctic_shift" | "arcticshift" => Ok(PostSourceKind::ArcticShift),
            "reddit_json" | "redditjson" | "reddit" => Ok(PostSourceKind::RedditJson),
            other => Err(anyhow!(
                "Unknown POST_SOURCE '{}' (expected 'arctic_shift' or 'reddit_json')",
                other
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    /// Backend used to discover new posts.
    pub post_source: PostSourceKind,
    /// Sustained request rate against the selected source.
    pub rate_limit_per_minute: u32,
    /// Maximum number of requests that may be sent back-to-back after idle time.
    pub rate_limit_burst: u32,
    /// Minimum time between the start of two poll cycles.
    pub poll_interval: Duration,
    /// Base URL of the Arctic Shift API (overridable for testing).
    pub arctic_shift_base_url: String,
    pub reddit_user_agent: String,
}

pub const DEFAULT_ARCTIC_SHIFT_BASE_URL: &str = "https://arctic-shift.photon-reddit.com";

/// Rate limit defaults and caps per source.
///
/// Reddit JSON: Reddit's unauthenticated limit is ~60/min; default 4, cap 45 (unchanged).
/// Arctic Shift: the maintainer asks for "a couple requests per second" at most and
/// rate limits dynamically; default 30/min (one every 2s), cap 120.
const REDDIT_DEFAULT_RATE: u32 = 4;
const REDDIT_MAX_RATE: u32 = 45;
const ARCTIC_SHIFT_DEFAULT_RATE: u32 = 30;
const ARCTIC_SHIFT_MAX_RATE: u32 = 120;
const ARCTIC_SHIFT_DEFAULT_POLL_INTERVAL_SECS: u64 = 30;

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Build the config from an arbitrary key lookup. Keeps parsing pure and testable
    /// without mutating process environment.
    pub fn from_lookup<F>(get: F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let database_url =
            get("DATABASE_URL").context("DATABASE_URL is required (e.g., sqlite://data.db)")?;

        let post_source = match get("POST_SOURCE") {
            Some(raw) if !raw.trim().is_empty() => raw.parse::<PostSourceKind>()?,
            _ => PostSourceKind::default(),
        };

        let (rate_limit_per_minute, rate_limit_burst, poll_interval) = match post_source {
            PostSourceKind::RedditJson => {
                let rate = parse_rate(
                    &get,
                    "REDDIT_RATE_LIMIT_PER_MINUTE",
                    REDDIT_DEFAULT_RATE,
                    REDDIT_MAX_RATE,
                );
                // Burst equals rate: identical to the historical token-bucket behaviour.
                let interval = parse_secs(&get, "POLL_INTERVAL_SECS", 0);
                (rate, rate, interval)
            }
            PostSourceKind::ArcticShift => {
                let rate = parse_rate(
                    &get,
                    "ARCTIC_SHIFT_RATE_LIMIT_PER_MINUTE",
                    ARCTIC_SHIFT_DEFAULT_RATE,
                    ARCTIC_SHIFT_MAX_RATE,
                );
                // Never burst into a dynamically rate-limited free service.
                let interval =
                    parse_secs(&get, "POLL_INTERVAL_SECS", ARCTIC_SHIFT_DEFAULT_POLL_INTERVAL_SECS);
                (rate, 1, interval)
            }
        };

        let arctic_shift_base_url = get("ARCTIC_SHIFT_BASE_URL")
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_ARCTIC_SHIFT_BASE_URL.to_string());

        let reddit_user_agent = get("REDDIT_USER_AGENT").unwrap_or_else(|| {
            format!(
                "reddit_notifier/{} (https://github.com/mandreko/reddit-notifier)",
                env!("CARGO_PKG_VERSION")
            )
        });

        Ok(Self {
            database_url,
            post_source,
            rate_limit_per_minute,
            rate_limit_burst,
            poll_interval,
            arctic_shift_base_url,
            reddit_user_agent,
        })
    }
}

/// Parse a requests-per-minute value, falling back to `default` when unset or invalid,
/// clamping to `1..=max` so the refill interval (60s / rate) is always well defined.
fn parse_rate<F>(get: &F, key: &str, default: u32, max: u32) -> u32
where
    F: Fn(&str) -> Option<String>,
{
    let requested = get(key)
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(default);

    if requested > max {
        tracing::warn!(
            "{} is set to {}, which exceeds the safe maximum of {}. Capping at {} req/min.",
            key,
            requested,
            max,
            max
        );
        max
    } else if requested == 0 {
        tracing::warn!("{} must be at least 1; using 1 req/min.", key);
        1
    } else {
        requested
    }
}

fn parse_secs<F>(get: &F, key: &str, default: u64) -> Duration
where
    F: Fn(&str) -> Option<String>,
{
    let secs = get(key)
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(default);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn post_source_kind_parses_canonical_names() {
        assert_eq!("arctic_shift".parse::<PostSourceKind>().unwrap(), PostSourceKind::ArcticShift);
        assert_eq!("reddit_json".parse::<PostSourceKind>().unwrap(), PostSourceKind::RedditJson);
    }

    #[test]
    fn post_source_kind_accepts_aliases_and_case() {
        for alias in ["Arctic-Shift", "ARCTICSHIFT", " arctic_shift "] {
            assert_eq!(alias.parse::<PostSourceKind>().unwrap(), PostSourceKind::ArcticShift, "{alias}");
        }
        for alias in ["reddit", "Reddit-JSON", "redditjson"] {
            assert_eq!(alias.parse::<PostSourceKind>().unwrap(), PostSourceKind::RedditJson, "{alias}");
        }
    }

    #[test]
    fn post_source_kind_rejects_unknown() {
        let err = "rss".parse::<PostSourceKind>().unwrap_err();
        assert!(err.to_string().contains("rss"));
    }

    #[test]
    fn post_source_kind_as_str_round_trips() {
        for kind in [PostSourceKind::ArcticShift, PostSourceKind::RedditJson] {
            assert_eq!(kind.as_str().parse::<PostSourceKind>().unwrap(), kind);
        }
    }

    #[test]
    fn database_url_is_required() {
        let err = AppConfig::from_lookup(lookup(&[])).unwrap_err();
        assert!(err.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn defaults_to_arctic_shift() {
        let cfg = AppConfig::from_lookup(lookup(&[("DATABASE_URL", "sqlite://x.db")])).unwrap();
        assert_eq!(cfg.post_source, PostSourceKind::ArcticShift);
        assert_eq!(cfg.rate_limit_per_minute, ARCTIC_SHIFT_DEFAULT_RATE);
        assert_eq!(cfg.rate_limit_burst, 1);
        assert_eq!(cfg.poll_interval, Duration::from_secs(ARCTIC_SHIFT_DEFAULT_POLL_INTERVAL_SECS));
        assert_eq!(cfg.arctic_shift_base_url, DEFAULT_ARCTIC_SHIFT_BASE_URL);
        assert!(cfg.reddit_user_agent.starts_with("reddit_notifier/"));
    }

    #[test]
    fn empty_post_source_uses_default() {
        let cfg = AppConfig::from_lookup(lookup(&[("DATABASE_URL", "sqlite://x.db"), ("POST_SOURCE", "  ")])).unwrap();
        assert_eq!(cfg.post_source, PostSourceKind::ArcticShift);
    }

    #[test]
    fn reddit_json_defaults_match_legacy_behaviour() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("POST_SOURCE", "reddit_json"),
        ]))
        .unwrap();
        assert_eq!(cfg.post_source, PostSourceKind::RedditJson);
        assert_eq!(cfg.rate_limit_per_minute, REDDIT_DEFAULT_RATE);
        assert_eq!(cfg.rate_limit_burst, REDDIT_DEFAULT_RATE);
        assert_eq!(cfg.poll_interval, Duration::ZERO);
    }

    #[test]
    fn reddit_rate_is_capped_and_arctic_var_ignored() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("POST_SOURCE", "reddit_json"),
            ("REDDIT_RATE_LIMIT_PER_MINUTE", "500"),
            ("ARCTIC_SHIFT_RATE_LIMIT_PER_MINUTE", "7"),
        ]))
        .unwrap();
        assert_eq!(cfg.rate_limit_per_minute, REDDIT_MAX_RATE);
        assert_eq!(cfg.rate_limit_burst, REDDIT_MAX_RATE);
    }

    #[test]
    fn arctic_rate_is_capped_and_reddit_var_ignored() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("ARCTIC_SHIFT_RATE_LIMIT_PER_MINUTE", "999"),
            ("REDDIT_RATE_LIMIT_PER_MINUTE", "7"),
        ]))
        .unwrap();
        assert_eq!(cfg.rate_limit_per_minute, ARCTIC_SHIFT_MAX_RATE);
        assert_eq!(cfg.rate_limit_burst, 1);
    }

    #[test]
    fn zero_or_garbage_rate_falls_back_safely() {
        let zero = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("ARCTIC_SHIFT_RATE_LIMIT_PER_MINUTE", "0"),
        ]))
        .unwrap();
        assert_eq!(zero.rate_limit_per_minute, 1);

        let garbage = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("ARCTIC_SHIFT_RATE_LIMIT_PER_MINUTE", "fast"),
        ]))
        .unwrap();
        assert_eq!(garbage.rate_limit_per_minute, ARCTIC_SHIFT_DEFAULT_RATE);
    }

    #[test]
    fn poll_interval_override_and_garbage() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("POLL_INTERVAL_SECS", "120"),
        ]))
        .unwrap();
        assert_eq!(cfg.poll_interval, Duration::from_secs(120));

        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("POLL_INTERVAL_SECS", "soon"),
        ]))
        .unwrap();
        assert_eq!(cfg.poll_interval, Duration::from_secs(ARCTIC_SHIFT_DEFAULT_POLL_INTERVAL_SECS));
    }

    #[test]
    fn invalid_post_source_is_an_error() {
        let err = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("POST_SOURCE", "pushshift"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("POST_SOURCE"));
    }

    #[test]
    fn base_url_override_strips_trailing_slash() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("ARCTIC_SHIFT_BASE_URL", "http://127.0.0.1:8080/"),
        ]))
        .unwrap();
        assert_eq!(cfg.arctic_shift_base_url, "http://127.0.0.1:8080");
    }

    #[test]
    fn user_agent_override() {
        let cfg = AppConfig::from_lookup(lookup(&[
            ("DATABASE_URL", "sqlite://x.db"),
            ("REDDIT_USER_AGENT", "custom/1.0"),
        ]))
        .unwrap();
        assert_eq!(cfg.reddit_user_agent, "custom/1.0");
    }
}
