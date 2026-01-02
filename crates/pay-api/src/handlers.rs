//! # Request Handlers
//!
//! Axum request handlers for the payment API.

use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use pay_core::{Currency, LineItem, Order, PaymentError};
use pay_stripe::{dispatch_webhook_event, LoggingWebhookHandler};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

// =============================================================================
// Request/Response Types
// =============================================================================

/// Create checkout request
#[derive(Debug, Deserialize)]
pub struct CreateCheckoutRequest {
    /// Items to purchase
    pub items: Vec<CheckoutItem>,
    /// Customer email (optional)
    #[serde(default)]
    pub customer_email: Option<String>,
    /// Payment provider (optional, defaults to "stripe")
    #[serde(default)]
    pub provider: Option<String>,
    /// Idempotency key (optional)
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// Item in checkout request
#[derive(Debug, Deserialize)]
pub struct CheckoutItem {
    /// Product ID
    pub product_id: String,
    /// Quantity
    #[serde(default = "default_quantity")]
    pub quantity: u32,
}

fn default_quantity() -> u32 {
    1
}

/// Create checkout response
#[derive(Debug, Serialize)]
pub struct CreateCheckoutResponse {
    /// Session ID
    pub session_id: String,
    /// Checkout URL (redirect user here)
    pub checkout_url: String,
    /// Session expiration time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, code: u16) -> Self {
        Self {
            error: error.into(),
            code,
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}


fn payment_error_to_response(err: PaymentError) -> (StatusCode, Json<ErrorResponse>) {
    let code = err.status_code();
    let response = ErrorResponse::new(err.to_string(), code);
    (StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(response))
}

// =============================================================================
// Handlers
// =============================================================================

/// Health check endpoint
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "lightning-cart",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Create a checkout session
#[instrument(skip(state, request), fields(items = request.items.len()))]
pub async fn create_checkout(
    State(state): State<AppState>,
    Json(request): Json<CreateCheckoutRequest>,
) -> Result<Json<CreateCheckoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate request
    if request.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("No items in checkout request", 400)),
        ));
    }

    // Get payment strategy
    let provider = request.provider.as_deref();
    let strategy = state.strategies.get_or_default(provider).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                format!("Unknown payment provider: {:?}", provider),
                400,
            )),
        )
    })?;

    // Build order
    let mut order = Order::new(Currency::USD);

    if let Some(email) = &request.customer_email {
        order.customer_email = Some(email.clone());
    }

    if let Some(key) = &request.idempotency_key {
        order.idempotency_key = Some(key.clone());
    }

    // Add line items
    for item in &request.items {
        let product = state.catalog.get(&item.product_id).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    format!("Product not found: {}", item.product_id),
                    404,
                )),
            )
        })?;

        if !product.active {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    format!("Product is not available: {}", item.product_id),
                    400,
                )),
            ));
        }

        order.add_item(LineItem::from_product(product, item.quantity));
    }

    info!(
        "Creating checkout: {} items, total={}",
        order.item_count(),
        order.total().display()
    );

    // Create checkout session
    let session = strategy
    .create_checkout(&order, &state.success_url(), &state.cancel_url())
    .await
    .map_err(|e| {
        error!("Failed to create checkout: {}", e);
        payment_error_to_response(e)
    })?;

    info!("Created checkout session: {}", session.session_id);

    Ok(Json(CreateCheckoutResponse {
        session_id: session.session_id,
        checkout_url: session.checkout_url,
        expires_at: session.expires_at.map(|t| t.to_rfc3339()),
    }))
}

/// Handle Stripe webhook
#[instrument(skip(state, headers, body))]
pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    // Get signature header
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("Missing Stripe-Signature header", 400)),
            )
        })?;

    // Get Stripe strategy
    let strategy = state.strategies.get("stripe").ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("Stripe not configured", 500)),
        )
    })?;

    // Verify and parse webhook
    let event = strategy
    .verify_webhook(&body, signature)
    .await
    .map_err(|e| {
        error!("Webhook verification failed: {}", e);
        payment_error_to_response(e)
    })?;

    info!(
        "Received webhook: type={:?}, id={}",
        event.event_type, event.event_id
    );

    // Dispatch to handler
    // In production, you'd implement a custom WebhookHandler
    let handler = LoggingWebhookHandler;
    dispatch_webhook_event(&handler, event).map_err(|e| {
    error!("Webhook handler error: {}", e);
    payment_error_to_response(e)
    })?;

    Ok(StatusCode::OK)
}

/// Get products list
pub async fn list_products(State(state): State<AppState>) -> impl IntoResponse {
    let products: Vec<_> = state.catalog.active_products().collect();
    Json(serde_json::json!({
        "products": products,
        "count": products.len()
    }))
}

/// Get single product
pub async fn get_product(
    State(state): State<AppState>,
    Path(product_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let product = state.catalog.get(&product_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                format!("Product not found: {}", product_id),
                404,
            )),
        )
    })?;

    Ok(Json(product.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response() {
        let err = ErrorResponse::new("Test error", 400);
        assert_eq!(err.error, "Test error");
        assert_eq!(err.code, 400);
    }

    #[test]
    fn test_payment_error_conversion() {
        let err = PaymentError::InvalidRequest("Bad data".to_string());
        let (status, _json) = <(StatusCode, Json<ErrorResponse>)>::from(err);
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
