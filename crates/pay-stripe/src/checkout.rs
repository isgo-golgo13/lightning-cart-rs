//! # Stripe Checkout Sessions
//!
//! Implementation of Stripe Checkout Sessions API.
//! This is the primary payment flow for lightning-cart.

use crate::config::StripeConfig;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use pay_core::{
    BillingInterval, CheckoutMode, CheckoutSession, CheckoutStatus, Order, PaymentError,
    PaymentResult, PaymentStrategy, WebhookEvent, WebhookEventType,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument};

/// Stripe Checkout Session strategy
///
/// Uses Stripe's hosted checkout page for secure payments.
/// This is the recommended approach for PCI compliance.
pub struct StripeCheckoutStrategy {
    config: StripeConfig,
    client: Client,
}

impl StripeCheckoutStrategy {
    /// Create a new Stripe checkout strategy
    pub fn new(config: StripeConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Create from environment variables
    pub fn from_env() -> PaymentResult<Self> {
        let config = StripeConfig::from_env()?;
        Ok(Self::new(config))
    }

    /// Build line items for Stripe API
    fn build_line_items(&self, order: &Order) -> Vec<StripeLineItem> {
        order
            .line_items
            .iter()
            .map(|item| {
                let recurring = match item.billing_interval {
                    BillingInterval::OneTime => None,
                    BillingInterval::Weekly => Some(StripeRecurring {
                        interval: "week".to_string(),
                        interval_count: 1,
                    }),
                    BillingInterval::Monthly => Some(StripeRecurring {
                        interval: "month".to_string(),
                        interval_count: 1,
                    }),
                    BillingInterval::Yearly => Some(StripeRecurring {
                        interval: "year".to_string(),
                        interval_count: 1,
                    }),
                };

                StripeLineItem {
                    price_data: StripePriceData {
                        currency: item.unit_price.currency.as_str().to_string(),
                        unit_amount: item.unit_price.amount,
                        product_data: StripeProductData {
                            name: item.name.clone(),
                            description: item.description.clone(),
                            images: item.image_url.clone().map(|url| vec![url]),
                        },
                        recurring,
                    },
                    quantity: item.quantity as i64,
                }
            })
            .collect()
    }

    /// Convert our checkout mode to Stripe's mode
    fn stripe_mode(mode: CheckoutMode) -> &'static str {
        match mode {
            CheckoutMode::Payment => "payment",
            CheckoutMode::Subscription => "subscription",
            CheckoutMode::Setup => "setup",
        }
    }
}

#[async_trait]
impl PaymentStrategy for StripeCheckoutStrategy {
    #[instrument(skip(self, order), fields(order_id = %order.id))]
    async fn create_checkout(
        &self,
        order: &Order,
        success_url: &str,
        cancel_url: &str,
    ) -> PaymentResult<CheckoutSession> {
        if order.is_empty() {
            return Err(PaymentError::InvalidRequest(
                "Order has no items".to_string(),
            ));
        }

        let line_items = self.build_line_items(order);
        let mode = Self::stripe_mode(order.mode);

        debug!(
            "Creating Stripe checkout session: {} items, mode={}",
            line_items.len(),
            mode
        );

        // Build form data for Stripe API
        let mut form_params: Vec<(String, String)> = vec![
            ("mode".to_string(), mode.to_string()),
            ("success_url".to_string(), success_url.to_string()),
            ("cancel_url".to_string(), cancel_url.to_string()),
        ];

        // Add line items
        for (i, item) in line_items.iter().enumerate() {
            form_params.push((
                format!("line_items[{}][price_data][currency]", i),
                item.price_data.currency.clone(),
            ));
            form_params.push((
                format!("line_items[{}][price_data][unit_amount]", i),
                item.price_data.unit_amount.to_string(),
            ));
            form_params.push((
                format!("line_items[{}][price_data][product_data][name]", i),
                item.price_data.product_data.name.clone(),
            ));
            if let Some(ref desc) = item.price_data.product_data.description {
                form_params.push((
                    format!("line_items[{}][price_data][product_data][description]", i),
                    desc.clone(),
                ));
            }
            if let Some(ref images) = item.price_data.product_data.images {
                for (j, img) in images.iter().enumerate() {
                    form_params.push((
                        format!("line_items[{}][price_data][product_data][images][{}]", i, j),
                        img.clone(),
                    ));
                }
            }
            if let Some(ref recurring) = item.price_data.recurring {
                form_params.push((
                    format!("line_items[{}][price_data][recurring][interval]", i),
                    recurring.interval.clone(),
                ));
                form_params.push((
                    format!("line_items[{}][price_data][recurring][interval_count]", i),
                    recurring.interval_count.to_string(),
                ));
            }
            form_params.push((
                format!("line_items[{}][quantity]", i),
                item.quantity.to_string(),
            ));
        }

        // Add customer email if provided
        if let Some(ref email) = order.customer_email {
            form_params.push(("customer_email".to_string(), email.clone()));
        }

        // Add idempotency key
        let idempotency_key = order
            .idempotency_key
            .clone()
            .unwrap_or_else(|| order.id.clone());

        // Add metadata
        form_params.push(("metadata[order_id]".to_string(), order.id.clone()));
        for (key, value) in &order.metadata {
            form_params.push((format!("metadata[{}]", key), value.clone()));
        }

        let url = format!("{}/v1/checkout/sessions", self.config.api_base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.config.auth_header())
            .header("Stripe-Version", &self.config.api_version)
            .header("Idempotency-Key", &idempotency_key)
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

            // Parse Stripe error
            if let Ok(error_response) = serde_json::from_str::<StripeErrorResponse>(&body) {
                return Err(PaymentError::ProviderError {
                    provider: "stripe".to_string(),
                    message: error_response.error.message,
                });
            }

            return Err(PaymentError::ProviderError {
                provider: "stripe".to_string(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let session_response: StripeCheckoutSessionResponse =
            serde_json::from_str(&body).map_err(|e| {
                PaymentError::Serialization(format!(
                    "Failed to parse Stripe response: {}",
                    e
                ))
            })?;

        info!(
            "Created Stripe checkout session: id={}, url={}",
            session_response.id, session_response.url
        );

        let expires_at = session_response
            .expires_at
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or(Utc::now() + Duration::hours(24)));

        Ok(CheckoutSession {
            session_id: session_response.id,
            order_id: order.id.clone(),
            provider: "stripe".to_string(),
            checkout_url: session_response.url,
            status: CheckoutStatus::Open,
            expires_at,
            payment_intent_id: session_response.payment_intent,
            customer_id: session_response.customer,
            created_at: Utc::now(),
        })
    }

    #[instrument(skip(self, payload, signature))]
    async fn verify_webhook(
        &self,
        payload: &[u8],
        signature: &str,
    ) -> PaymentResult<WebhookEvent> {
        // Parse signature header
        let sig_parts = parse_signature_header(signature)?;

        // Verify timestamp is within tolerance (5 minutes)
        let timestamp = sig_parts.timestamp;
        let now = Utc::now().timestamp();
        let tolerance = 300; // 5 minutes

        if (now - timestamp).abs() > tolerance {
            return Err(PaymentError::WebhookVerificationFailed(
                "Timestamp outside tolerance".to_string(),
            ));
        }

        // Compute expected signature
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let expected_sig = compute_hmac_sha256(&self.config.webhook_secret, &signed_payload);

        // Compare signatures (constant-time)
        let valid = sig_parts
            .signatures
            .iter()
            .any(|sig| constant_time_compare(sig, &expected_sig));

        if !valid {
            return Err(PaymentError::WebhookVerificationFailed(
                "Signature mismatch".to_string(),
            ));
        }

        // Parse the event
        let event: StripeWebhookEvent = serde_json::from_slice(payload).map_err(|e| {
            PaymentError::WebhookParseError(format!("Failed to parse webhook: {}", e))
        })?;

        debug!("Verified Stripe webhook: type={}", event.event_type);

        let event_type = match event.event_type.as_str() {
            "checkout.session.completed" => WebhookEventType::CheckoutCompleted,
            "payment_intent.succeeded" => WebhookEventType::PaymentSucceeded,
            "payment_intent.payment_failed" => WebhookEventType::PaymentFailed,
            "customer.subscription.created" => WebhookEventType::SubscriptionCreated,
            "customer.subscription.deleted" => WebhookEventType::SubscriptionCancelled,
            "invoice.paid" => WebhookEventType::SubscriptionRenewed,
            "charge.refunded" => WebhookEventType::RefundIssued,
            other => WebhookEventType::Unknown(other.to_string()),
        };

        // Extract relevant fields from the event data
        let session_id = event
            .data
            .object
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let payment_intent_id = event
            .data
            .object
            .get("payment_intent")
            .and_then(|v| v.as_str())
            .map(String::from);

        let customer_email = event
            .data
            .object
            .get("customer_details")
            .and_then(|cd| cd.get("email"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let amount_paid = event
            .data
            .object
            .get("amount_total")
            .and_then(|v| v.as_i64());

        Ok(WebhookEvent {
            event_id: event.id,
            event_type,
            provider: "stripe".to_string(),
            session_id,
            payment_intent_id,
            customer_email,
            amount_paid,
            currency: None, // Could parse from event if needed
            raw_data: Some(serde_json::Value::Object(event.data.object)),
            timestamp: DateTime::from_timestamp(event.created, 0).unwrap_or(Utc::now()),
        })
    }

    fn provider_name(&self) -> &'static str {
        "stripe"
    }

    fn supports_subscriptions(&self) -> bool {
        true
    }
}

// =============================================================================
// Stripe API Types
// =============================================================================

#[derive(Debug, Serialize)]
struct StripeLineItem {
    price_data: StripePriceData,
    quantity: i64,
}

#[derive(Debug, Serialize)]
struct StripePriceData {
    currency: String,
    unit_amount: i64,
    product_data: StripeProductData,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurring: Option<StripeRecurring>,
}

#[derive(Debug, Serialize)]
struct StripeProductData {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct StripeRecurring {
    interval: String,
    interval_count: i64,
}

#[derive(Debug, Deserialize)]
struct StripeCheckoutSessionResponse {
    id: String,
    url: String,
    #[serde(default)]
    payment_intent: Option<String>,
    #[serde(default)]
    customer: Option<String>,
    #[serde(default)]
    expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct StripeErrorResponse {
    error: StripeError,
}

#[derive(Debug, Deserialize)]
struct StripeError {
    message: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    param: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StripeWebhookEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    created: i64,
    data: StripeEventData,
}

#[derive(Debug, Deserialize)]
struct StripeEventData {
    object: serde_json::Map<String, serde_json::Value>,
}

// =============================================================================
// Webhook Signature Verification
// =============================================================================

struct SignatureHeader {
    timestamp: i64,
    signatures: Vec<String>,
}

fn parse_signature_header(header: &str) -> PaymentResult<SignatureHeader> {
    let mut timestamp = None;
    let mut signatures = Vec::new();

    for part in header.split(',') {
        let kv: Vec<&str> = part.split('=').collect();
        if kv.len() != 2 {
            continue;
        }
        match kv[0] {
            "t" => {
                timestamp = kv[1].parse().ok();
            }
            "v1" => {
                signatures.push(kv[1].to_string());
            }
            _ => {}
        }
    }

    let timestamp = timestamp.ok_or_else(|| {
        PaymentError::WebhookVerificationFailed("Missing timestamp in signature".to_string())
    })?;

    if signatures.is_empty() {
        return Err(PaymentError::WebhookVerificationFailed(
            "No v1 signature found".to_string(),
        ));
    }

    Ok(SignatureHeader {
        timestamp,
        signatures,
    })
}

fn compute_hmac_sha256(secret: &str, message: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use pay_core::{Currency, LineItem, Price};

    #[test]
    fn test_stripe_mode_conversion() {
        assert_eq!(
            StripeCheckoutStrategy::stripe_mode(CheckoutMode::Payment),
            "payment"
        );
        assert_eq!(
            StripeCheckoutStrategy::stripe_mode(CheckoutMode::Subscription),
            "subscription"
        );
        assert_eq!(
            StripeCheckoutStrategy::stripe_mode(CheckoutMode::Setup),
            "setup"
        );
    }

    #[test]
    fn test_parse_signature_header() {
        let header = "t=1234567890,v1=abc123,v1=def456";
        let parsed = parse_signature_header(header).unwrap();

        assert_eq!(parsed.timestamp, 1234567890);
        assert_eq!(parsed.signatures.len(), 2);
        assert_eq!(parsed.signatures[0], "abc123");
    }

    #[test]
    fn test_hmac_sha256() {
        let secret = "whsec_test";
        let message = "1234567890.{}";
        let sig = compute_hmac_sha256(secret, message);

        // Should produce a 64-character hex string
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("abc123", "abc123"));
        assert!(!constant_time_compare("abc123", "abc124"));
        assert!(!constant_time_compare("abc", "abcd"));
    }
}
