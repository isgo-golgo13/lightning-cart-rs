//! # Request Handlers
//!
//! Axum request handlers for the payment API.
//! Supports multi-tenant checkout with site-specific URLs and statement descriptors.

use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use pay_core::{Currency, LineItem, Order, PaymentError};
use pay_stripe::{dispatch_webhook_event, CheckoutCompletedData, LoggingWebhookHandler};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

// =============================================================================
// Request/Response Types
// =============================================================================

/// Create checkout request
#[derive(Debug, Deserialize)]
pub struct CreateCheckoutRequest {
    /// Items to purchase
    #[serde(default)]
    pub items: Vec<CheckoutItem>,
    /// Convenience: single product_id (alternative to items array for single-product checkout)
    #[serde(default)]
    pub product_id: Option<String>,
    /// Customer email (optional)
    #[serde(default)]
    pub customer_email: Option<String>,
    /// Payment provider (optional, defaults to "stripe")
    #[serde(default)]
    pub provider: Option<String>,
    /// Idempotency key (optional)
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// Site ID for multi-tenant (optional, can also be in URL path)
    #[serde(default)]
    pub site_id: Option<String>,
    /// Custom metadata to pass through to Stripe (e.g., consultation booking details)
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
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

/// Create a checkout session (legacy route - uses default site)
#[instrument(skip(state, request), fields(items = request.items.len()))]
pub async fn create_checkout(
    State(state): State<AppState>,
    Json(request): Json<CreateCheckoutRequest>,
) -> Result<Json<CreateCheckoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Use site_id from request body, or default to chargegun
    let site_id = request.site_id.clone();
    create_checkout_internal(&state, request, site_id.as_deref()).await
}

/// Create a checkout session for a specific site (multi-tenant route)
#[instrument(skip(state, request), fields(site_id = %site_id, items = request.items.len()))]
pub async fn create_checkout_for_site(
    State(state): State<AppState>,
    Path(site_id): Path<String>,
    Json(request): Json<CreateCheckoutRequest>,
) -> Result<Json<CreateCheckoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate site exists
    if !state.sites.has_site(&site_id) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(format!("Site not found: {}", site_id), 404)),
        ));
    }

    create_checkout_internal(&state, request, Some(&site_id)).await
}

/// Internal checkout creation (shared logic)
async fn create_checkout_internal(
    state: &AppState,
    request: CreateCheckoutRequest,
    site_id: Option<&str>,
) -> Result<Json<CreateCheckoutResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Support single product_id as shorthand for items array
    let items = if !request.items.is_empty() {
        request.items
    } else if let Some(ref pid) = request.product_id {
        vec![CheckoutItem {
            product_id: pid.clone(),
            quantity: 1,
        }]
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("No items in checkout request (provide 'items' array or 'product_id')", 400)),
        ));
    };

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

    // Add site_id to order metadata for webhook processing
    if let Some(sid) = site_id {
        order.metadata.insert("site_id".to_string(), sid.to_string());
    }

    // Add statement descriptor suffix to metadata for Stripe
    if let Some(descriptor) = state.statement_descriptor_for_site(site_id) {
        order.metadata.insert("statement_descriptor_suffix".to_string(), descriptor);
    }

    // Merge request metadata into order (consultation booking details, etc.)
    for (key, value) in &request.metadata {
        order.metadata.insert(key.clone(), value.clone());
    }

    // Add line items
    for item in &items {
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

    // Get site-specific URLs
    let success_url = state.success_url_for_site(site_id);
    let cancel_url = state.cancel_url_for_site(site_id);

    info!(
        "Creating checkout: site={:?}, {} items, total={}, success_url={}",
        site_id,
        order.item_count(),
        order.total().display(),
        success_url
    );

    // Create checkout session
    let session = strategy
        .create_checkout(&order, &success_url, &cancel_url)
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

    // Extract site_id from event metadata if present
    let site_id = event
        .raw_data
        .as_ref()
        .and_then(|d| d.get("metadata"))
        .and_then(|m| m.get("site_id"))
        .and_then(|v| v.as_str());

    if let Some(sid) = site_id {
        info!("Webhook for site: {}", sid);
    }

    // Extract consultation data BEFORE dispatch consumes the event
    let consultation_forward = if matches!(&event.event_type, pay_core::WebhookEventType::CheckoutCompleted) {
        match CheckoutCompletedData::from_event(&event) {
            Ok(data) if data.metadata.contains_key("appointment_date") => {
                let forward_site = data.metadata.get("site_id").cloned()
                    .unwrap_or_else(|| "chargegun".to_string());
                Some((forward_site, data))
            }
            _ => None,
        }
    } else {
        None
    };

    // Dispatch to existing handler (unchanged — LoggingWebhookHandler just logs)
    let handler = LoggingWebhookHandler;
    dispatch_webhook_event(&handler, event).map_err(|e| {
        error!("Webhook handler error: {}", e);
        payment_error_to_response(e)
    })?;

    // === Forward consultation bookings to Vercel ===
    if let Some((forward_site, data)) = consultation_forward {
        if let Some(webhook_url) = state.webhook_forward_urls.get(&forward_site) {
            info!(
                "Forwarding consultation to Vercel: site={}, payment={:?}, amount={}",
                forward_site, data.payment_intent_id, data.amount_total
            );

            // Build payload matching Vercel consultation-webhook.js expectations
            let payload = serde_json::json!({
                "firstName": data.metadata.get("client_first_name").cloned().unwrap_or_default(),
                "lastName": data.metadata.get("client_last_name").cloned().unwrap_or_default(),
                "email": data.metadata.get("client_email").cloned().unwrap_or_default(),
                "appointmentDate": data.metadata.get("appointment_date").cloned().unwrap_or_default(),
                "appointmentTime": data.metadata.get("appointment_time").cloned().unwrap_or_default(),
                "duration": data.metadata.get("duration").and_then(|d| d.parse::<i32>().ok()).unwrap_or(1),
                "amountCents": data.amount_total,
                "stripePaymentId": data.payment_intent_id.clone().unwrap_or_else(|| "unknown".to_string()),
            });

            match state.http_client
                .post(webhook_url)
                .json(&payload)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        info!("Vercel webhook success: {} | {}", status, body);
                    } else {
                        error!("Vercel webhook error: {} | {}", status, body);
                    }
                }
                Err(e) => {
                    error!("Failed to forward to Vercel: {}", e);
                    // Don't fail the Stripe webhook — we received the event successfully.
                    // The Vercel call can be retried manually if needed.
                }
            }
        } else {
            info!("No webhook_forward_url configured for site: {}", forward_site);
        }
    }

    Ok(StatusCode::OK)
}

/// Get products list (all sites)
pub async fn list_products(State(state): State<AppState>) -> impl IntoResponse {
    let products: Vec<_> = state.catalog.active_products().collect();
    Json(serde_json::json!({
        "products": products,
        "count": products.len()
    }))
}

/// Get products for a specific site
pub async fn list_products_for_site(
    State(state): State<AppState>,
    Path(site_id): Path<String>,
) -> impl IntoResponse {
    let products: Vec<_> = state.catalog.active_products_for_site(&site_id).collect();
    Json(serde_json::json!({
        "site_id": site_id,
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

/// List all registered sites
pub async fn list_sites(State(state): State<AppState>) -> impl IntoResponse {
    let sites: Vec<_> = state.sites.active_sites().collect();
    Json(serde_json::json!({
        "sites": sites,
        "count": sites.len()
    }))
}

/// Get single site info
pub async fn get_site(
    State(state): State<AppState>,
    Path(site_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let site = state.sites.get(&site_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                format!("Site not found: {}", site_id),
                404,
            )),
        )
    })?;

    Ok(Json(site.clone()))
}

/// Checkout success page
pub async fn checkout_success(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let session_id = params.get("session_id").map(|s| s.as_str()).unwrap_or("unknown");
    axum::response::Html(format!(r#"
<!DOCTYPE html>
<html>
<head><title>Payment Successful</title></head>
<body style="font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);">
    <div style="background: white; padding: 60px; border-radius: 16px; text-align: center;">
        <div style="font-size: 60px;">✅</div>
        <h1>Payment Successful!</h1>
        <p>Session: <code>{}</code></p>
        <p style="color: #666;">Your payment was processed successfully.</p>
    </div>
</body>
</html>
"#, session_id))
}

/// Checkout cancel page  
pub async fn checkout_cancel() -> impl IntoResponse {
    axum::response::Html(r#"
<!DOCTYPE html>
<html>
<head><title>Payment Cancelled</title></head>
<body style="font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);">
    <div style="background: white; padding: 60px; border-radius: 16px; text-align: center;">
        <div style="font-size: 60px;">❌</div>
        <h1>Payment Cancelled</h1>
        <p style="color: #666;">No charges were made.</p>
    </div>
</body>
</html>
"#)
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
        let (status, _json) = payment_error_to_response(err);
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
