//! # Payment Strategy Trait
//!
//! Core Strategy pattern trait for payment providers.
//! Implementations: Stripe, PayPal, Square, etc.
//!
//! ## Design Pattern
//!
//! This uses the Strategy design pattern to allow swapping payment providers
//! at runtime without changing client code. Each provider implements the
//! `PaymentStrategy` trait.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    PaymentStrategy (trait)                  │
//! │  ├── create_checkout()                                      │
//! │  ├── verify_webhook()                                       │
//! │  └── provider_name()                                        │
//! └─────────────────────────────────────────────────────────────┘
//!                            ▲
//!          ┌─────────────────┼─────────────────┐
//!          │                 │                 │
//!  ┌───────┴───────┐ ┌───────┴───────┐ ┌───────┴───────┐
//!  │StripeCheckout │ │ PayPalStrategy│ │ SquareStrategy│
//!  │   Strategy    │ │   (future)    │ │   (future)    │
//!  └───────────────┘ └───────────────┘ └───────────────┘
//! ```

use crate::error::PaymentResult;
use crate::order::{CheckoutSession, Order, WebhookEvent};
use async_trait::async_trait;
use std::sync::Arc;

/// Core trait for payment provider implementations.
///
/// Each payment provider (Stripe, PayPal, Square) implements this trait,
/// allowing the application to switch providers via configuration.
#[async_trait]
pub trait PaymentStrategy: Send + Sync {
    /// Create a checkout session and return the redirect URL.
    ///
    /// # Arguments
    /// * `order` - The order to check out
    /// * `success_url` - URL to redirect after successful payment
    /// * `cancel_url` - URL to redirect if customer cancels
    ///
    /// # Returns
    /// A `CheckoutSession` containing the redirect URL and session details.
    async fn create_checkout(
        &self,
        order: &Order,
        success_url: &str,
        cancel_url: &str,
    ) -> PaymentResult<CheckoutSession>;

    /// Verify a webhook signature and parse the event.
    ///
    /// # Arguments
    /// * `payload` - Raw webhook body bytes
    /// * `signature` - Signature header from the request
    ///
    /// # Returns
    /// A parsed `WebhookEvent` if signature is valid.
    async fn verify_webhook(
        &self,
        payload: &[u8],
        signature: &str,
    ) -> PaymentResult<WebhookEvent>;

    /// Get the provider name (for logging and routing).
    fn provider_name(&self) -> &'static str;

    /// Check if this provider supports subscriptions.
    fn supports_subscriptions(&self) -> bool {
        true
    }

    /// Get the webhook endpoint path for this provider.
    /// Default: `/webhook/{provider_name}`
    fn webhook_path(&self) -> String {
        format!("/webhook/{}", self.provider_name())
    }
}

/// Type alias for a boxed payment strategy (dynamic dispatch)
pub type BoxedPaymentStrategy = Arc<dyn PaymentStrategy>;

/// Strategy selector for multiple providers
#[derive(Clone)]
pub struct PaymentStrategySelector {
    strategies: std::collections::HashMap<String, BoxedPaymentStrategy>,
    default_provider: String,
}

impl PaymentStrategySelector {
    /// Create a new selector with a default provider
    pub fn new(default_provider: impl Into<String>) -> Self {
        Self {
            strategies: std::collections::HashMap::new(),
            default_provider: default_provider.into(),
        }
    }

    /// Register a payment strategy
    pub fn register(&mut self, strategy: BoxedPaymentStrategy) {
        let name = strategy.provider_name().to_string();
        self.strategies.insert(name, strategy);
    }

    /// Register with builder pattern
    pub fn with_strategy(mut self, strategy: BoxedPaymentStrategy) -> Self {
        self.register(strategy);
        self
    }

    /// Get the default strategy
    pub fn default_strategy(&self) -> Option<&BoxedPaymentStrategy> {
        self.strategies.get(&self.default_provider)
    }

    /// Get a strategy by provider name
    pub fn get(&self, provider: &str) -> Option<&BoxedPaymentStrategy> {
        self.strategies.get(provider)
    }

    /// Get strategy or fall back to default
    pub fn get_or_default(&self, provider: Option<&str>) -> Option<&BoxedPaymentStrategy> {
        match provider {
            Some(p) => self.get(p).or_else(|| self.default_strategy()),
            None => self.default_strategy(),
        }
    }

    /// List all registered providers
    pub fn providers(&self) -> Vec<&str> {
        self.strategies.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider is registered
    pub fn has_provider(&self, provider: &str) -> bool {
        self.strategies.contains_key(provider)
    }
}

impl Default for PaymentStrategySelector {
    fn default() -> Self {
        Self::new("stripe")
    }
}

/// Configuration for URLs used in checkout
#[derive(Debug, Clone)]
pub struct CheckoutUrls {
    /// Base URL of the application (e.g., "https://enginevector.io")
    pub base_url: String,
    /// Success page path (e.g., "/checkout/success")
    pub success_path: String,
    /// Cancel page path (e.g., "/checkout/cancel")
    pub cancel_path: String,
}

impl CheckoutUrls {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            success_path: "/checkout/success".to_string(),
            cancel_path: "/checkout/cancel".to_string(),
        }
    }

    pub fn success_url(&self) -> String {
        format!("{}{}", self.base_url, self.success_path)
    }

    pub fn cancel_url(&self) -> String {
        format!("{}{}", self.base_url, self.cancel_path)
    }

    pub fn with_session_id(&self, session_id: &str) -> (String, String) {
        (
            format!("{}?session_id={}", self.success_url(), session_id),
            format!("{}?session_id={}", self.cancel_url(), session_id),
        )
    }
}

impl Default for CheckoutUrls {
    fn default() -> Self {
        Self::new("http://localhost:3000")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkout_urls() {
        let urls = CheckoutUrls::new("https://enginevector.io");

        assert_eq!(urls.success_url(), "https://enginevector.io/checkout/success");
        assert_eq!(urls.cancel_url(), "https://enginevector.io/checkout/cancel");
    }

    #[test]
    fn test_strategy_selector() {
        let selector = PaymentStrategySelector::new("stripe");

        assert_eq!(selector.providers().len(), 0);
        assert!(selector.default_strategy().is_none());
    }
}
