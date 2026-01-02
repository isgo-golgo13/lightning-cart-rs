//! # pay-stripe
//!
//! Stripe payment strategy for lightning-cart-rs.
//!
//! This crate provides two strategies for accepting payments via Stripe:
//!
//! 1. **StripeCheckoutStrategy** - Full Checkout Sessions API
//!    - Dynamic line items
//!    - Customer email prefill
//!    - Metadata support
//!    - Best for: e-commerce, dynamic pricing
//!
//! 2. **StripeLinksStrategy** - Payment Links API
//!    - Pre-configured products
//!    - Shareable URLs
//!    - Best for: fixed-price products, social selling
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use pay_stripe::StripeCheckoutStrategy;
//! use pay_core::{Order, Currency, PaymentStrategy};
//!
//! // Create strategy from environment
//! let strategy = StripeCheckoutStrategy::from_env()?;
//!
//! // Create checkout session
//! let session = strategy.create_checkout(
//!     &order,
//!     "https://example.com/success",
//!     "https://example.com/cancel",
//! ).await?;
//!
//! // Redirect user to session.checkout_url
//! ```
//!
//! ## Webhook Handling
//!
//! ```rust,ignore
//! use pay_stripe::{StripeCheckoutStrategy, webhook::{WebhookHandler, dispatch_webhook_event}};
//!
//! struct MyHandler;
//!
//! impl WebhookHandler for MyHandler {
//!     fn on_checkout_completed(&self, data: CheckoutCompletedData) -> PaymentResult<()> {
//!         // Fulfill the order
//!         println!("Order {} paid!", data.order_id().unwrap_or("unknown"));
//!         Ok(())
//!     }
//! }
//!
//! // In your webhook endpoint:
//! let event = strategy.verify_webhook(payload, signature).await?;
//! dispatch_webhook_event(&MyHandler, event)?;
//! ```

pub mod checkout;
pub mod config;
pub mod links;
pub mod webhook;

// Re-exports
pub use checkout::StripeCheckoutStrategy;
pub use config::StripeConfig;
pub use links::{PaymentLinkResponse, StripeLinksStrategy};
pub use webhook::{
    dispatch_webhook_event, CheckoutCompletedData, LoggingWebhookHandler, WebhookHandler,
    REQUIRED_WEBHOOK_EVENTS,
};
