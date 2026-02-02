//! # Routes
//!
//! Axum router configuration for the payment API.
//! Supports both legacy single-tenant and multi-tenant routes.

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
///
/// Routes:
/// - Legacy (backwards compatible):
///   - POST /api/v1/checkout - Create checkout (uses default site)
///   - GET  /api/v1/products - List all products
///   - GET  /api/v1/products/{id} - Get product by ID
///
/// - Multi-tenant:
///   - POST /api/v1/{site_id}/checkout - Create checkout for site
///   - GET  /api/v1/{site_id}/products - List products for site
///   - GET  /api/v1/sites - List all sites
///   - GET  /api/v1/sites/{site_id} - Get site info
///
/// - Webhooks:
///   - POST /webhook/stripe - Stripe webhook handler
///
/// - Static pages:
///   - GET /checkout/success - Success page
///   - GET /checkout/cancel - Cancel page
pub fn create_router(state: AppState) -> Router {
    // CORS configuration - allow all origins for now
    // In production, you might want to dynamically build this from the site registry
    let cors = CorsLayer::new()
        .allow_origin(Any) // TODO: In production, use site-specific origins
        .allow_methods(Any)
        .allow_headers(Any);

    // Static success/cancel pages
    let checkout_routes = Router::new()
        .route("/success", get(handlers::checkout_success))
        .route("/cancel", get(handlers::checkout_cancel));

    // Legacy API routes (backwards compatible - uses default site)
    let legacy_api_routes = Router::new()
        // Checkout
        .route("/checkout", post(handlers::create_checkout))
        // Products
        .route("/products", get(handlers::list_products))
        .route("/products/{product_id}", get(handlers::get_product));

    // Multi-tenant site routes
    let site_api_routes = Router::new()
        // Site-specific checkout
        .route("/{site_id}/checkout", post(handlers::create_checkout_for_site))
        // Site-specific products
        .route("/{site_id}/products", get(handlers::list_products_for_site))
        // Site management
        .route("/sites", get(handlers::list_sites))
        .route("/sites/{site_id}", get(handlers::get_site));

    // Combined API v1 routes
    let api_routes = Router::new()
        // Legacy routes first (more specific)
        .merge(legacy_api_routes)
        // Then multi-tenant routes
        .merge(site_api_routes);

    // Webhook routes (no CORS, must accept raw body)
    let webhook_routes = Router::new()
        .route("/stripe", post(handlers::stripe_webhook));

    // Combine all routes
    Router::new()
        // Health check at root
        .route("/health", get(handlers::health))
        .route("/", get(handlers::health))
        // Checkout success/cancel pages
        .nest("/checkout", checkout_routes)
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
        .route("/api/v1/{site_id}/checkout", post(handlers::create_checkout_for_site))
        .route("/api/v1/products", get(handlers::list_products))
        .route("/api/v1/{site_id}/products", get(handlers::list_products_for_site))
        .route("/api/v1/sites", get(handlers::list_sites))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    // Router tests would go here
    // Using axum-test crate for integration tests
}
