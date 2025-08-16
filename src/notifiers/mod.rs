use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

use crate::models::{DiscordConfig, EndpointRow, EndpointKind, PushoverConfig};

pub mod discord;
pub mod pushover;

#[async_trait]
pub trait Notifier: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn send(&self, subreddit: &str, title: &str, url: &str) -> Result<()>;
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
