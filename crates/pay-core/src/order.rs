//! # Order Types
//!
//! Order and checkout session types for lightning-cart.

use crate::product::{BillingInterval, Currency, Price, Product};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A line item in an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    /// Product ID
    pub product_id: String,

    /// Product name (denormalized for display)
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Unit price
    pub unit_price: Price,

    /// Quantity
    pub quantity: u32,

    /// Billing interval (for subscriptions)
    #[serde(default)]
    pub billing_interval: BillingInterval,

    /// Optional image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

impl LineItem {
    /// Create a line item from a product
    pub fn from_product(product: &Product, quantity: u32) -> Self {
        Self {
            product_id: product.id.clone(),
            name: product.name.clone(),
            description: Some(product.description.clone()),
            unit_price: product.price.clone(),
            quantity,
            billing_interval: product.billing_interval,
            image_url: product.image_url.clone(),
        }
    }

    /// Calculate the total price for this line item
    pub fn total(&self) -> Price {
        Price {
            amount: self.unit_price.amount * self.quantity as i64,
            currency: self.unit_price.currency,
        }
    }
}

/// Checkout mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutMode {
    /// One-time payment
    Payment,
    /// Subscription
    Subscription,
    /// Setup (save card for later)
    Setup,
}

impl Default for CheckoutMode {
    fn default() -> Self {
        CheckoutMode::Payment
    }
}

/// An order to be checked out
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order ID (generated)
    pub id: String,

    /// Line items
    pub line_items: Vec<LineItem>,

    /// Currency (must be same for all items)
    pub currency: Currency,

    /// Checkout mode
    #[serde(default)]
    pub mode: CheckoutMode,

    /// Customer email (optional, for prefill)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<String>,

    /// Idempotency key (prevents duplicate charges)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,

    /// Custom metadata
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order with generated ID
    pub fn new(currency: Currency) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            line_items: Vec::new(),
            currency,
            mode: CheckoutMode::Payment,
            customer_email: None,
            idempotency_key: Some(Uuid::new_v4().to_string()),
            metadata: std::collections::HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a line item
    pub fn add_item(&mut self, item: LineItem) {
        // Auto-detect subscription mode
        if !matches!(item.billing_interval, BillingInterval::OneTime) {
            self.mode = CheckoutMode::Subscription;
        }
        self.line_items.push(item);
    }

    /// Add a product with quantity
    pub fn add_product(&mut self, product: &Product, quantity: u32) {
        self.add_item(LineItem::from_product(product, quantity));
    }

    /// Calculate order total
    pub fn total(&self) -> Price {
        let total_amount: i64 = self.line_items.iter().map(|item| item.total().amount).sum();
        Price {
            amount: total_amount,
            currency: self.currency,
        }
    }

    /// Set customer email
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.customer_email = Some(email.into());
        self
    }

    /// Set idempotency key
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if order is empty
    pub fn is_empty(&self) -> bool {
        self.line_items.is_empty()
    }

    /// Get item count
    pub fn item_count(&self) -> u32 {
        self.line_items.iter().map(|i| i.quantity).sum()
    }
}

/// Status of a checkout session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutStatus {
    /// Session created, awaiting payment
    Open,
    /// Payment completed successfully
    Complete,
    /// Session expired
    Expired,
    /// Payment failed
    Failed,
    /// Customer cancelled
    Cancelled,
}

impl Default for CheckoutStatus {
    fn default() -> Self {
        CheckoutStatus::Open
    }
}

/// A checkout session created by a payment provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    /// Provider's session ID
    pub session_id: String,

    /// Our internal order ID
    pub order_id: String,

    /// Provider name (e.g., "stripe", "paypal")
    pub provider: String,

    /// URL to redirect customer to for payment
    pub checkout_url: String,

    /// Session status
    #[serde(default)]
    pub status: CheckoutStatus,

    /// When the session expires
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,

    /// Payment intent ID (Stripe-specific, but useful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_intent_id: Option<String>,

    /// Customer ID (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

impl CheckoutSession {
    /// Create a new checkout session
    pub fn new(
        session_id: impl Into<String>,
        order_id: impl Into<String>,
        provider: impl Into<String>,
        checkout_url: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            order_id: order_id.into(),
            provider: provider.into(),
            checkout_url: checkout_url.into(),
            status: CheckoutStatus::Open,
            expires_at: None,
            payment_intent_id: None,
            customer_id: None,
            created_at: Utc::now(),
        }
    }

    /// Check if session is still valid
    pub fn is_active(&self) -> bool {
        matches!(self.status, CheckoutStatus::Open)
            && self
                .expires_at
                .map(|exp| exp > Utc::now())
                .unwrap_or(true)
    }
}

/// Webhook event types we care about
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventType {
    /// Checkout session completed
    CheckoutCompleted,
    /// Payment succeeded
    PaymentSucceeded,
    /// Payment failed
    PaymentFailed,
    /// Subscription created
    SubscriptionCreated,
    /// Subscription cancelled
    SubscriptionCancelled,
    /// Subscription renewed
    SubscriptionRenewed,
    /// Refund issued
    RefundIssued,
    /// Unknown event (passthrough)
    Unknown(String),
}

/// A parsed webhook event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    /// Event ID from provider
    pub event_id: String,

    /// Event type
    pub event_type: WebhookEventType,

    /// Provider name
    pub provider: String,

    /// Related session ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Related payment intent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_intent_id: Option<String>,

    /// Customer email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<String>,

    /// Amount paid (in smallest unit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_paid: Option<i64>,

    /// Currency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Currency>,

    /// Raw event data (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_data: Option<serde_json::Value>,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::{Price, Product};

    #[test]
    fn test_line_item_total() {
        let product = Product::one_time("test", "Test", Price::new(10.0, Currency::USD));
        let item = LineItem::from_product(&product, 3);

        assert_eq!(item.total().amount, 3000); // $30.00 in cents
    }

    #[test]
    fn test_order_total() {
        let mut order = Order::new(Currency::USD);

        let product1 = Product::one_time("p1", "Product 1", Price::new(10.0, Currency::USD));
        let product2 = Product::one_time("p2", "Product 2", Price::new(25.0, Currency::USD));

        order.add_product(&product1, 2); // $20
        order.add_product(&product2, 1); // $25

        assert_eq!(order.total().amount, 4500); // $45.00
        assert_eq!(order.item_count(), 3);
    }

    #[test]
    fn test_subscription_mode_detection() {
        let mut order = Order::new(Currency::USD);

        let subscription = Product::subscription(
            "sub",
            "Monthly Sub",
            Price::new(29.0, Currency::USD),
            BillingInterval::Monthly,
        );

        order.add_product(&subscription, 1);

        assert_eq!(order.mode, CheckoutMode::Subscription);
    }

    #[test]
    fn test_checkout_session_active() {
        let session = CheckoutSession::new("sess_123", "ord_456", "stripe", "https://checkout.stripe.com/...");

        assert!(session.is_active());
        assert_eq!(session.status, CheckoutStatus::Open);
    }
}
