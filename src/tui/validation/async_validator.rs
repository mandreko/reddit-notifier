use async_trait::async_trait;

/// Result of an async validation
///
/// Ok(Some(msg)) = Valid with success message
/// Ok(None) = Valid without message
/// Err(msg) = Invalid with error message
pub type ValidationResult = Result<Option<String>, String>;

/// Trait for async field validation
///
/// Allows validation that requires async operations like HTTP requests,
/// database queries, or other I/O operations.
#[async_trait]
pub trait AsyncValidator: Send + Sync {
    /// Validate a value asynchronously
    ///
    /// Returns Ok(Some(message)) for valid input with a success message,
    /// Ok(None) for valid input without a message,
    /// or Err(message) for invalid input with an error message.
    async fn validate(&self, value: &str) -> ValidationResult;
}

/// Validator that checks if a field is not empty
pub struct NonEmptyValidator;

#[async_trait]
impl AsyncValidator for NonEmptyValidator {
    async fn validate(&self, value: &str) -> ValidationResult {
        if value.trim().is_empty() {
            Err("Field cannot be empty".to_string())
        } else {
            Ok(None)
        }
    }
}

/// Validator that checks if a URL is properly formatted
pub struct UrlFormatValidator;

#[async_trait]
impl AsyncValidator for UrlFormatValidator {
    async fn validate(&self, value: &str) -> ValidationResult {
        // Attempt to parse the URL
        match reqwest::Url::parse(value) {
            Ok(url) => {
                // Check that it has a valid scheme
                let scheme = url.scheme();
                if scheme == "http" || scheme == "https" {
                    Ok(None)
                } else {
                    Err(format!("URL must use http:// or https:// (found: {}://)", scheme))
                }
            }
            Err(e) => Err(format!("Invalid URL format: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_non_empty_validator_empty() {
        let validator = NonEmptyValidator;
        let result = validator.validate("").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Field cannot be empty");
    }

    #[tokio::test]
    async fn test_non_empty_validator_whitespace() {
        let validator = NonEmptyValidator;
        let result = validator.validate("   ").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_non_empty_validator_valid() {
        let validator = NonEmptyValidator;
        let result = validator.validate("test").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_url_format_validator_https() {
        let validator = UrlFormatValidator;
        let result = validator.validate("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_url_format_validator_http() {
        let validator = UrlFormatValidator;
        let result = validator.validate("http://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_url_format_validator_with_path() {
        let validator = UrlFormatValidator;
        let result = validator.validate("https://example.com/path/to/resource").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_url_format_validator_with_query() {
        let validator = UrlFormatValidator;
        let result = validator.validate("https://example.com/api?key=value&foo=bar").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_url_format_validator_invalid_scheme() {
        let validator = UrlFormatValidator;
        let result = validator.validate("ftp://example.com").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ftp://"));
    }

    #[tokio::test]
    async fn test_url_format_validator_malformed() {
        let validator = UrlFormatValidator;
        let result = validator.validate("not a url at all").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid URL format"));
    }

    #[tokio::test]
    async fn test_url_format_validator_missing_scheme() {
        let validator = UrlFormatValidator;
        let result = validator.validate("example.com").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid URL format"));
    }
}
