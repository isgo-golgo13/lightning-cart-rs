//! # Stripe Payment Links
//!
//! Implementation of Stripe Payment Links API.
//! Payment Links are pre-configured, reusable URLs for accepting payments.
//!
//! Use Payment Links when:
//! - You have fixed products/prices
//! - You want shareable links (email, social media)
//! - You don't need dynamic line items per checkout
//!
//! Use Checkout Sessions when:
//! - You need dynamic pricing/products
//! - You want full control over the checkout flow

use crate::config::StripeConfig;
use async_trait::async_trait;
use chrono::Utc;
use pay_core::{
    CheckoutSession, CheckoutStatus, Order, PaymentError, PaymentResult, PaymentStrategy,
    WebhookEvent,
};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, info, instrument};

/// Stripe Payment Links strategy
///
/// Uses pre-created Payment Links for simpler checkout flows.
/// Links must be created in Stripe Dashboard or via API beforehand.
pub struct StripeLinksStrategy {
    config: StripeConfig,
    client: Client,
    /// Map of product_id -> payment_link_id
    link_mappings: std::collections::HashMap<String, String>,
}

impl StripeLinksStrategy {
    /// Create a new Payment Links strategy
    pub fn new(config: StripeConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            client,
            link_mappings: std::collections::HashMap::new(),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> PaymentResult<Self> {
        let config = StripeConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Register a payment link for a product
    ///
    /// # Arguments
    /// * `product_id` - Your internal product ID
    /// * `payment_link_id` - Stripe Payment Link ID (plink_...)
    pub fn register_link(&mut self, product_id: impl Into<String>, payment_link_id: impl Into<String>) {
        self.link_mappings.insert(product_id.into(), payment_link_id.into());
    }

    /// Builder: register a payment link
    pub fn with_link(mut self, product_id: impl Into<String>, payment_link_id: impl Into<String>) -> Self {
        self.register_link(product_id, payment_link_id);
        self
    }

    /// Get a payment link URL for a product
    pub async fn get_link_url(&self, product_id: &str) -> PaymentResult<String> {
        let link_id = self.link_mappings.get(product_id).ok_or_else(|| {
            PaymentError::ProductNotFound {
                product_id: product_id.to_string(),
            }
        })?;

        // Fetch the Payment Link to get its URL
        let url = format!("{}/v1/payment_links/{}", self.config.api_base_url, link_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.config.auth_header())
            .header("Stripe-Version", &self.config.api_version)
            .send()
            .await
            .map_err(|e| PaymentError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| PaymentError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("Stripe API error: status={}, body={}", status, body);
            return Err(PaymentError::ProviderError {
                provider: "stripe".to_string(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let link_response: PaymentLinkResponse =
            serde_json::from_str(&body).map_err(|e| {
                PaymentError::Serialization(format!("Failed to parse response: {}", e))
            })?;

        Ok(link_response.url)
    }

    /// Create a new Payment Link via API
    ///
    /// This creates a reusable payment link that can be shared.
    #[instrument(skip(self))]
    pub async fn create_payment_link(
        &self,
        price_id: &str,
        quantity: i64,
    ) -> PaymentResult<PaymentLinkResponse> {
        let url = format!("{}/v1/payment_links", self.config.api_base_url);

        let form_params = vec![
            ("line_items[0][price]".to_string(), price_id.to_string()),
            ("line_items[0][quantity]".to_string(), quantity.to_string()),
        ];

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.config.auth_header())
            .header("Stripe-Version", &self.config.api_version)
            .form(&form_params)
            .send()
            .await
            .map_err(|e| PaymentError::NetworkError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| PaymentError::NetworkError(e.to_string()))?;

        if !status.is_success() {
            error!("Stripe API error: status={}, body={}", status, body);
            return Err(PaymentError::ProviderError {
                provider: "stripe".to_string(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let link_response: PaymentLinkResponse =
            serde_json::from_str(&body).map_err(|e| {
                PaymentError::Serialization(format!("Failed to parse response: {}", e))
            })?;

        info!("Created Payment Link: id={}, url={}", link_response.id, link_response.url);

        Ok(link_response)
    }
}

#[async_trait]
impl PaymentStrategy for StripeLinksStrategy {
    #[instrument(skip(self, order), fields(order_id = %order.id))]
    async fn create_checkout(
        &self,
        order: &Order,
        _success_url: &str,
        _cancel_url: &str,
    ) -> PaymentResult<CheckoutSession> {
        // For Payment Links, we only support single-product orders
        if order.line_items.len() != 1 {
            return Err(PaymentError::InvalidRequest(
                "Payment Links only support single-product orders. Use Checkout Sessions for multiple items.".to_string()
            ));
        }

        let item = &order.line_items[0];
        let checkout_url = self.get_link_url(&item.product_id).await?;

        debug!("Using Payment Link for product: {}", item.product_id);

        // Note: Payment Links don't return a session ID until checkout is complete
        // We generate our own tracking ID
        Ok(CheckoutSession {
            session_id: format!("plink_{}", order.id),
            order_id: order.id.clone(),
            provider: "stripe_links".to_string(),
            checkout_url,
            status: CheckoutStatus::Open,
            expires_at: None, // Payment Links don't expire
            payment_intent_id: None,
            customer_id: None,
            created_at: Utc::now(),
        })
    }

    async fn verify_webhook(
        &self,
        _payload: &[u8],
        _signature: &str,
    ) -> PaymentResult<WebhookEvent> {
        // Payment Links use the same webhook format as Checkout Sessions
        // Delegate to the main Stripe checkout implementation
        Err(PaymentError::Internal(
            "Use StripeCheckoutStrategy for webhook verification".to_string()
        ))
    }

    fn provider_name(&self) -> &'static str {
        "stripe_links"
    }

    fn supports_subscriptions(&self) -> bool {
        true // Payment Links support subscriptions if configured with recurring prices
    }
}

// =============================================================================
// Stripe API Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct PaymentLinkResponse {
    pub id: String,
    pub url: String,
    pub active: bool,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_link() {
        let config = StripeConfig::new("sk_test_abc", "pk_test_xyz", "whsec_123");
        let strategy = StripeLinksStrategy::new(config)
            .with_link("rang-play-rs-cli", "plink_abc123")
            .with_link("site-ranker-rs-cli", "plink_def456");

        assert!(strategy.link_mappings.contains_key("rang-play-rs-cli"));
        assert!(strategy.link_mappings.contains_key("site-ranker-rs-cli"));
    }
}
