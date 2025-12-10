use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

use crate::models::database::EndpointKind;

use super::async_validator::{AsyncValidator, ValidationResult};

/// Validator for webhook endpoints
///
/// Sends a test message to verify the webhook is valid and reachable.
/// Supports Discord and Pushover endpoints.
pub struct WebhookValidator {
    client: Client,
    endpoint_kind: EndpointKind,
}

impl WebhookValidator {
    /// Create a new webhook validator for the given endpoint kind
    pub fn new(endpoint_kind: EndpointKind) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            endpoint_kind,
        }
    }

    /// Validate a Discord webhook URL by sending a test message
    async fn validate_discord(&self, webhook_url: &str) -> ValidationResult {
        // Check URL format first
        if !webhook_url.starts_with("https://discord.com/api/webhooks/")
            && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
        {
            return Err("Invalid Discord webhook URL format".to_string());
        }

        let test_payload = json!({
            "content": "✅ Test message from reddit-notifier (validating webhook)",
            "username": "reddit-notifier"
        });

        match self
            .client
            .post(webhook_url)
            .json(&test_payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                Ok(Some("✓ Webhook is valid and reachable".to_string()))
            }
            Ok(resp) => Err(format!(
                "Webhook returned status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )),
            Err(e) => Err(format!("Cannot reach webhook: {}", e)),
        }
    }

    /// Validate Pushover configuration by checking token and user
    async fn validate_pushover(&self, config_json: &str) -> ValidationResult {
        // Parse the config JSON to extract token and user
        let config: serde_json::Value = match serde_json::from_str(config_json) {
            Ok(v) => v,
            Err(e) => return Err(format!("Invalid JSON: {}", e)),
        };

        let token = match config.get("token").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return Err("Missing 'token' field in configuration".to_string()),
        };

        let user = match config.get("user").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return Err("Missing 'user' field in configuration".to_string()),
        };

        // Validate with Pushover API
        let params = [
            ("token", token),
            ("user", user),
            ("message", "Test from reddit-notifier (validating credentials)"),
        ];

        match self
            .client
            .post("https://api.pushover.net/1/messages.json")
            .form(&params)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                Ok(Some("✓ Pushover credentials are valid".to_string()))
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Err(format!("Pushover API returned {}: {}", status, body))
            }
            Err(e) => Err(format!("Cannot reach Pushover API: {}", e)),
        }
    }
}

#[async_trait]
impl AsyncValidator for WebhookValidator {
    async fn validate(&self, value: &str) -> ValidationResult {
        match self.endpoint_kind {
            EndpointKind::Discord => self.validate_discord(value).await,
            EndpointKind::Pushover => self.validate_pushover(value).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discord_invalid_url_format() {
        let validator = WebhookValidator::new(EndpointKind::Discord);
        let result = validator.validate("https://example.com/webhook").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Invalid Discord webhook URL format"));
    }

    #[tokio::test]
    async fn test_discord_valid_url_format_unreachable() {
        let validator = WebhookValidator::new(EndpointKind::Discord);
        // Valid format but likely unreachable
        let result = validator
            .validate("https://discord.com/api/webhooks/123/abc")
            .await;
        // Should either fail with network error or invalid webhook
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pushover_invalid_json() {
        let validator = WebhookValidator::new(EndpointKind::Pushover);
        let result = validator.validate("not json").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_pushover_missing_token() {
        let validator = WebhookValidator::new(EndpointKind::Pushover);
        let result = validator.validate(r#"{"user": "test"}"#).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("token"));
    }

    #[tokio::test]
    async fn test_pushover_missing_user() {
        let validator = WebhookValidator::new(EndpointKind::Pushover);
        let result = validator.validate(r#"{"token": "test"}"#).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("user"));
    }
}
