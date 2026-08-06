use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

use crate::models::{
    database::{EndpointKind, EndpointRow},
    notifiers::{DiscordConfig, PushoverConfig},
};

pub mod discord;
pub mod pushover;

#[async_trait]
pub trait Notifier: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn send(&self, subreddit: &str, title: &str, url: &str) -> Result<()>;
}

/// Strip the request URL from a reqwest error before it can reach logs.
///
/// Webhook URLs are bearer credentials (anyone holding one can post to the
/// channel), and reqwest's error Display includes the full request URL.
pub(crate) fn redact_request_error(e: reqwest::Error) -> anyhow::Error {
    anyhow::Error::from(e.without_url())
}

pub fn build_notifier(row: &EndpointRow, client: Client) -> Result<Box<dyn Notifier>> {
    match row.kind {
        EndpointKind::Discord => {
            let cfg: DiscordConfig = serde_json::from_str(&row.config_json)?;
            Ok(Box::new(discord::DiscordNotifier { client, cfg }))
        }
        EndpointKind::Pushover => {
            let cfg: PushoverConfig = serde_json::from_str(&row.config_json)?;
            Ok(Box::new(pushover::PushoverNotifier { client, cfg }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn redact_request_error_strips_url() {
        // Provoke a real reqwest error carrying a URL: connect to a closed port
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let secret_url = "http://127.0.0.1:9/webhook/secret-token";
        let err = client.get(secret_url).send().await.unwrap_err();
        assert!(
            format!("{}", err).contains("secret-token"),
            "precondition: raw reqwest error should contain the URL"
        );

        let redacted = redact_request_error(err);
        // {:#} prints the whole anyhow chain
        let chain = format!("{:#}", redacted);
        assert!(
            !chain.contains("secret-token") && !chain.contains("webhook"),
            "redacted error must not contain the URL, got: {}",
            chain
        );
    }
}
