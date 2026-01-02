//! # Stripe Webhook Handling
//!
//! Utilities for handling Stripe webhooks.
//! Webhooks notify your server of events (payments completed, subscriptions changed, etc.)

use pay_core::{Currency, PaymentError, PaymentResult, WebhookEvent, WebhookEventType};
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Parsed checkout.session.completed event data
#[derive(Debug, Clone)]
pub struct CheckoutCompletedData {
    pub session_id: String,
    pub payment_intent_id: Option<String>,
    pub subscription_id: Option<String>,
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    pub amount_total: i64,
    pub currency: Currency,
    pub payment_status: String,
    pub metadata: std::collections::HashMap<String, String>,
}

impl CheckoutCompletedData {
    /// Parse from a webhook event
    pub fn from_event(event: &WebhookEvent) -> PaymentResult<Self> {
        let raw = event.raw_data.as_ref().ok_or_else(|| {
            PaymentError::WebhookParseError("Missing raw data".to_string())
        })?;

        let obj = raw.as_object().ok_or_else(|| {
            PaymentError::WebhookParseError("Raw data is not an object".to_string())
        })?;

        let session_id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| PaymentError::WebhookParseError("Missing session id".to_string()))?;

        let payment_intent_id = obj
            .get("payment_intent")
            .and_then(|v| v.as_str())
            .map(String::from);

        let subscription_id = obj
            .get("subscription")
            .and_then(|v| v.as_str())
            .map(String::from);

        let customer_id = obj
            .get("customer")
            .and_then(|v| v.as_str())
            .map(String::from);

        let customer_email = obj
            .get("customer_details")
            .and_then(|cd| cd.get("email"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let amount_total = obj
            .get("amount_total")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let currency_str = obj
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("usd");

        let currency = match currency_str.to_lowercase().as_str() {
            "usd" => Currency::USD,
            "eur" => Currency::EUR,
            "gbp" => Currency::GBP,
            "jpy" => Currency::JPY,
            "cad" => Currency::CAD,
            "aud" => Currency::AUD,
            "chf" => Currency::CHF,
            "mxn" => Currency::MXN,
            _ => Currency::USD,
        };

        let payment_status = obj
            .get("payment_status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let metadata = obj
            .get("metadata")
            .and_then(|m| m.as_object())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            session_id,
            payment_intent_id,
            subscription_id,
            customer_id,
            customer_email,
            amount_total,
            currency,
            payment_status,
            metadata,
        })
    }

    /// Check if payment was successful
    pub fn is_paid(&self) -> bool {
        self.payment_status == "paid"
    }

    /// Get the internal order ID from metadata
    pub fn order_id(&self) -> Option<&str> {
        self.metadata.get("order_id").map(|s| s.as_str())
    }
}

/// Webhook event handler trait
///
/// Implement this trait to handle different webhook events.
#[allow(unused_variables)]
pub trait WebhookHandler: Send + Sync {
    /// Called when a checkout session is completed
    fn on_checkout_completed(&self, data: CheckoutCompletedData) -> PaymentResult<()> {
        info!(
            "Checkout completed: session={}, amount={}",
            data.session_id, data.amount_total
        );
        Ok(())
    }

    /// Called when a payment succeeds
    fn on_payment_succeeded(&self, event: &WebhookEvent) -> PaymentResult<()> {
        info!("Payment succeeded: {:?}", event.payment_intent_id);
        Ok(())
    }

    /// Called when a payment fails
    fn on_payment_failed(&self, event: &WebhookEvent) -> PaymentResult<()> {
        warn!("Payment failed: {:?}", event.payment_intent_id);
        Ok(())
    }

    /// Called when a subscription is created
    fn on_subscription_created(&self, event: &WebhookEvent) -> PaymentResult<()> {
        info!("Subscription created: {:?}", event.session_id);
        Ok(())
    }

    /// Called when a subscription is cancelled
    fn on_subscription_cancelled(&self, event: &WebhookEvent) -> PaymentResult<()> {
        info!("Subscription cancelled: {:?}", event.session_id);
        Ok(())
    }

    /// Called when a subscription renews
    fn on_subscription_renewed(&self, event: &WebhookEvent) -> PaymentResult<()> {
        info!("Subscription renewed: {:?}", event.session_id);
        Ok(())
    }

    /// Called when a refund is issued
    fn on_refund_issued(&self, event: &WebhookEvent) -> PaymentResult<()> {
        info!("Refund issued: {:?}", event.payment_intent_id);
        Ok(())
    }

    /// Called for unknown/unhandled events
    fn on_unknown_event(&self, event: &WebhookEvent) -> PaymentResult<()> {
        debug!("Unhandled webhook event: {:?}", event.event_type);
        Ok(())
    }
}

/// Default no-op webhook handler (just logs events)
pub struct LoggingWebhookHandler;

impl WebhookHandler for LoggingWebhookHandler {}

/// Dispatch a webhook event to the appropriate handler method
pub fn dispatch_webhook_event(
    handler: &dyn WebhookHandler,
    event: WebhookEvent,
) -> PaymentResult<()> {
    match &event.event_type {
        WebhookEventType::CheckoutCompleted => {
            let data = CheckoutCompletedData::from_event(&event)?;
            handler.on_checkout_completed(data)
        }
        WebhookEventType::PaymentSucceeded => handler.on_payment_succeeded(&event),
        WebhookEventType::PaymentFailed => handler.on_payment_failed(&event),
        WebhookEventType::SubscriptionCreated => handler.on_subscription_created(&event),
        WebhookEventType::SubscriptionCancelled => handler.on_subscription_cancelled(&event),
        WebhookEventType::SubscriptionRenewed => handler.on_subscription_renewed(&event),
        WebhookEventType::RefundIssued => handler.on_refund_issued(&event),
        WebhookEventType::Unknown(_) => handler.on_unknown_event(&event),
    }
}

/// Events that should be enabled in Stripe Dashboard for full functionality
pub const REQUIRED_WEBHOOK_EVENTS: &[&str] = &[
    "checkout.session.completed",
    "checkout.session.expired",
    "payment_intent.succeeded",
    "payment_intent.payment_failed",
    "customer.subscription.created",
    "customer.subscription.updated",
    "customer.subscription.deleted",
    "invoice.paid",
    "invoice.payment_failed",
    "charge.refunded",
];

/// Print instructions for setting up webhooks
pub fn print_webhook_setup_instructions(endpoint_url: &str) {
    println!("=== Stripe Webhook Setup ===\n");
    println!("1. Go to: https://dashboard.stripe.com/webhooks\n");
    println!("2. Click 'Add endpoint'\n");
    println!("3. Enter endpoint URL: {}\n", endpoint_url);
    println!("4. Select these events:");
    for event in REQUIRED_WEBHOOK_EVENTS {
        println!("   - {}", event);
    }
    println!("\n5. Copy the signing secret (whsec_...) to your .env file");
    println!("\n6. For local testing, use Stripe CLI:");
    println!("   stripe listen --forward-to {}", endpoint_url);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    fn mock_checkout_event() -> WebhookEvent {
        WebhookEvent {
            event_id: "evt_test".to_string(),
            event_type: WebhookEventType::CheckoutCompleted,
            provider: "stripe".to_string(),
            session_id: Some("cs_test".to_string()),
            payment_intent_id: Some("pi_test".to_string()),
            customer_email: Some("test@example.com".to_string()),
            amount_paid: Some(1000),
            currency: Some(Currency::USD),
            raw_data: Some(json!({
                "id": "cs_test_123",
                "payment_intent": "pi_test_456",
                "customer": "cus_test_789",
                "customer_details": {
                    "email": "test@example.com"
                },
                "amount_total": 1000,
                "currency": "usd",
                "payment_status": "paid",
                "metadata": {
                    "order_id": "ord_test_abc"
                }
            })),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_parse_checkout_completed() {
        let event = mock_checkout_event();
        let data = CheckoutCompletedData::from_event(&event).unwrap();

        assert_eq!(data.session_id, "cs_test_123");
        assert_eq!(data.payment_intent_id, Some("pi_test_456".to_string()));
        assert_eq!(data.customer_email, Some("test@example.com".to_string()));
        assert_eq!(data.amount_total, 1000);
        assert!(data.is_paid());
        assert_eq!(data.order_id(), Some("ord_test_abc"));
    }

    #[test]
    fn test_dispatch_webhook() {
        struct TestHandler {
            called: std::sync::atomic::AtomicBool,
        }

        impl WebhookHandler for TestHandler {
            fn on_checkout_completed(&self, _data: CheckoutCompletedData) -> PaymentResult<()> {
                self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        let handler = TestHandler {
            called: std::sync::atomic::AtomicBool::new(false),
        };

        let event = mock_checkout_event();
        dispatch_webhook_event(&handler, event).unwrap();

        assert!(handler.called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
