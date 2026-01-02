//! # Routes
//!
//! Axum router configuration for the payment API.

use crate::handlers;
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any) // In production, restrict this
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes
    let api_routes = Router::new()
        // Checkout
        .route("/checkout", post(handlers::create_checkout))
        // Products
        .route("/products", get(handlers::list_products))
        .route("/products/:product_id", get(handlers::get_product));

    // Webhook routes (no CORS, must accept raw body)
    let webhook_routes = Router::new()
        .route("/stripe", post(handlers::stripe_webhook));

    // Combine all routes
    Router::new()
        // Health check at root
        .route("/health", get(handlers::health))
        .route("/", get(handlers::health))
        // API v1
        .nest("/api/v1", api_routes)
        // Webhooks
        .nest("/webhook", webhook_routes)
        // Middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        // State
        .with_state(state)
}

/// Create a minimal router for testing
#[cfg(test)]
pub fn create_test_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/checkout", post(handlers::create_checkout))
        .route("/api/v1/products", get(handlers::list_products))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    // Router tests would go here
    // Using axum-test crate for integration tests
}
