//! # pay-api
//!
//! HTTP API layer for lightning-cart-rs.
//!
//! This crate provides:
//! - Axum-based HTTP server
//! - REST endpoints for checkout and products
//! - Webhook handlers for payment events
//!
//! ## Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/health` | Health check |
//! | POST | `/api/v1/checkout` | Create checkout session |
//! | GET | `/api/v1/products` | List products |
//! | GET | `/api/v1/products/:id` | Get product |
//! | POST | `/webhook/stripe` | Stripe webhook |

pub mod handlers;
pub mod routes;
pub mod state;

pub use routes::create_router;
pub use state::{AppConfig, AppState};
