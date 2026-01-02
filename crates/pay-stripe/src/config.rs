//! # Stripe Configuration
//!
//! Configuration management for Stripe integration.
//! All secrets are loaded from environment variables.

use pay_core::PaymentError;
use std::env;

/// Stripe API configuration
#[derive(Debug, Clone)]
pub struct StripeConfig {
    /// Secret API key (sk_test_... or sk_live_...)
    pub secret_key: String,

    /// Publishable key (pk_test_... or pk_live_...)
    pub publishable_key: String,

    /// Webhook signing secret (whsec_...)
    pub webhook_secret: String,

    /// API base URL (for testing/mocking)
    pub api_base_url: String,

    /// API version
    pub api_version: String,
}

impl StripeConfig {
    /// Load configuration from environment variables.
    ///
    /// Required env vars:
    /// - `STRIPE_SECRET_KEY`
    /// - `STRIPE_PUBLISHABLE_KEY`
    /// - `STRIPE_WEBHOOK_SECRET`
    pub fn from_env() -> Result<Self, PaymentError> {
        dotenvy::dotenv().ok(); // Load .env file if present

        let secret_key = env::var("STRIPE_SECRET_KEY").map_err(|_| {
            PaymentError::Configuration("STRIPE_SECRET_KEY not set".to_string())
        })?;

        let publishable_key = env::var("STRIPE_PUBLISHABLE_KEY").map_err(|_| {
            PaymentError::Configuration("STRIPE_PUBLISHABLE_KEY not set".to_string())
        })?;

        let webhook_secret = env::var("STRIPE_WEBHOOK_SECRET").map_err(|_| {
            PaymentError::Configuration("STRIPE_WEBHOOK_SECRET not set".to_string())
        })?;

        // Validate key formats
        if !secret_key.starts_with("sk_test_") && !secret_key.starts_with("sk_live_") {
            return Err(PaymentError::Configuration(
                "STRIPE_SECRET_KEY must start with sk_test_ or sk_live_".to_string(),
            ));
        }

        if !publishable_key.starts_with("pk_test_") && !publishable_key.starts_with("pk_live_") {
            return Err(PaymentError::Configuration(
                "STRIPE_PUBLISHABLE_KEY must start with pk_test_ or pk_live_".to_string(),
            ));
        }

        if !webhook_secret.starts_with("whsec_") {
            return Err(PaymentError::Configuration(
                "STRIPE_WEBHOOK_SECRET must start with whsec_".to_string(),
            ));
        }

        Ok(Self {
            secret_key,
            publishable_key,
            webhook_secret,
            api_base_url: "https://api.stripe.com".to_string(),
            api_version: "2024-12-18.acacia".to_string(),
        })
    }

    /// Create config with explicit values (for testing)
    pub fn new(
        secret_key: impl Into<String>,
        publishable_key: impl Into<String>,
        webhook_secret: impl Into<String>,
    ) -> Self {
        Self {
            secret_key: secret_key.into(),
            publishable_key: publishable_key.into(),
            webhook_secret: webhook_secret.into(),
            api_base_url: "https://api.stripe.com".to_string(),
            api_version: "2024-12-18.acacia".to_string(),
        }
    }

    /// Check if using test keys
    pub fn is_test_mode(&self) -> bool {
        self.secret_key.starts_with("sk_test_")
    }

    /// Check if using live keys
    pub fn is_live_mode(&self) -> bool {
        self.secret_key.starts_with("sk_live_")
    }

    /// Get authorization header value
    pub fn auth_header(&self) -> String {
        format!("Bearer {}", self.secret_key)
    }

    /// Builder: set custom API base URL (for testing)
    pub fn with_api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base_url = url.into();
        self
    }
}

impl Default for StripeConfig {
    fn default() -> Self {
        Self::from_env().expect("Failed to load Stripe config from environment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_validation() {
        // Valid test keys
        let config = StripeConfig::new(
            "sk_test_abc123",
            "pk_test_xyz789",
            "whsec_secret",
        );
        assert!(config.is_test_mode());
        assert!(!config.is_live_mode());

        // Valid live keys
        let config = StripeConfig::new(
            "sk_live_abc123",
            "pk_live_xyz789",
            "whsec_secret",
        );
        assert!(!config.is_test_mode());
        assert!(config.is_live_mode());
    }

    #[test]
    fn test_auth_header() {
        let config = StripeConfig::new(
            "sk_test_abc123",
            "pk_test_xyz789",
            "whsec_secret",
        );
        assert_eq!(config.auth_header(), "Bearer sk_test_abc123");
    }

    #[test]
    fn test_from_env_missing_key() {
        // Clear any existing env vars
        env::remove_var("STRIPE_SECRET_KEY");
        
        let result = StripeConfig::from_env();
        assert!(result.is_err());
    }
}
