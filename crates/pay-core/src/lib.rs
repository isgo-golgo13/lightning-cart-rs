//! # pay-core
//!
//! Core types and traits for the lightning-cart payment engine.
//!
//! This crate provides:
//! - `PaymentStrategy` trait for implementing payment providers
//! - `Product` and `ProductCatalog` for the product catalog
//! - `Order`, `LineItem`, and `CheckoutSession` for checkout flow
//! - `PaymentError` for typed error handling
//!
//! ## Example
//!
//! ```rust,ignore
//! use pay_core::{Order, Product, Price, Currency, PaymentStrategy};
//!
//! // Create an order
//! let mut order = Order::new(Currency::USD);
//!
//! // Add products
//! let product = Product::one_time("rang-play-rs", "Rang Play RS", Price::new(29.99, Currency::USD));
//! order.add_product(&product, 1);
//!
//! // Create checkout session using a strategy
//! let session = strategy.create_checkout(&order, success_url, cancel_url).await?;
//!
//! // Redirect user to session.checkout_url
//! ```

pub mod error;
pub mod order;
pub mod product;
pub mod strategy;

// Re-exports for convenience
pub use error::{PaymentError, PaymentResult};
pub use order::{
    CheckoutMode, CheckoutSession, CheckoutStatus, LineItem, Order, WebhookEvent,
    WebhookEventType,
};
pub use product::{
    BillingInterval, Currency, Price, Product, ProductCatalog, ProductType,
};
pub use strategy::{
    BoxedPaymentStrategy, CheckoutUrls, PaymentStrategy, PaymentStrategySelector,
};
