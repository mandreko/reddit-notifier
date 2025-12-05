use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use html_escape::decode_html_entities;

use crate::models::notifiers::DiscordConfig;
use super::Notifier;

pub struct DiscordNotifier {
    pub client: Client,
    pub cfg: DiscordConfig,
}

#[async_trait]
impl Notifier for DiscordNotifier {
    fn kind(&self) -> &'static str {
        "discord"
    }

    async fn send(&self, subreddit: &str, title: &str, url: &str) -> Result<()> {
        let payload = serde_json::json!({
            "username": self.cfg.username.as_deref().unwrap_or("Reddit Notifier"),
            "embeds": [{
                "title": format!("New Reddit Post Alert ({})", subreddit),
                "description": decode_html_entities(title),
                "url": url,
                "type": "rich"
            }]
        });
        let res = self.client.post(&self.cfg.webhook_url).json(&payload).send().await?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("discord webhook non-success: {} body: {}", status, body);
        }
        Ok(())
    }
}
