//! # Payment Error Types
//!
//! Typed error handling for the lightning-cart payment engine.
//! All payment operations return `Result<T, PaymentError>`.

use thiserror::Error;

/// Core error type for all payment operations
#[derive(Debug, Error)]
pub enum PaymentError {
    /// Configuration errors (missing keys, invalid config)
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Invalid request data
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Product not found in catalog
    #[error("Product not found: {product_id}")]
    ProductNotFound { product_id: String },

    /// Price mismatch or invalid amount
    #[error("Invalid price: {message}")]
    InvalidPrice { message: String },

    /// Currency not supported
    #[error("Unsupported currency: {currency}")]
    UnsupportedCurrency { currency: String },

    /// Payment provider API error
    #[error("Provider error [{provider}]: {message}")]
    ProviderError { provider: String, message: String },

    /// Network/HTTP error communicating with provider
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Webhook signature verification failed
    #[error("Webhook verification failed: {0}")]
    WebhookVerificationFailed(String),

    /// Webhook payload parsing error
    #[error("Webhook parse error: {0}")]
    WebhookParseError(String),

    /// Checkout session creation failed
    #[error("Checkout creation failed: {0}")]
    CheckoutCreationFailed(String),

    /// Session expired or not found
    #[error("Session not found or expired: {session_id}")]
    SessionNotFound { session_id: String },

    /// Payment was declined
    #[error("Payment declined: {reason}")]
    PaymentDeclined { reason: String },

    /// Idempotency conflict (duplicate request with different params)
    #[error("Idempotency conflict: request with key {key} already exists with different parameters")]
    IdempotencyConflict { key: String },

    /// Rate limited by provider
    #[error("Rate limited by {provider}, retry after {retry_after_secs} seconds")]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },

    /// Internal error (should not happen)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl PaymentError {
    /// Returns true if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PaymentError::NetworkError(_)
                | PaymentError::RateLimited { .. }
                | PaymentError::ProviderError { .. }
        )
    }

    /// Returns the HTTP status code appropriate for this error
    pub fn status_code(&self) -> u16 {
        match self {
            PaymentError::Configuration(_) => 500,
            PaymentError::InvalidRequest(_) => 400,
            PaymentError::ProductNotFound { .. } => 404,
            PaymentError::InvalidPrice { .. } => 400,
            PaymentError::UnsupportedCurrency { .. } => 400,
            PaymentError::ProviderError { .. } => 502,
            PaymentError::NetworkError(_) => 503,
            PaymentError::WebhookVerificationFailed(_) => 401,
            PaymentError::WebhookParseError(_) => 400,
            PaymentError::CheckoutCreationFailed(_) => 500,
            PaymentError::SessionNotFound { .. } => 404,
            PaymentError::PaymentDeclined { .. } => 402,
            PaymentError::IdempotencyConflict { .. } => 409,
            PaymentError::RateLimited { .. } => 429,
            PaymentError::Internal(_) => 500,
            PaymentError::Serialization(_) => 500,
        }
    }
}

/// Result type alias for payment operations
pub type PaymentResult<T> = Result<T, PaymentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        assert!(PaymentError::NetworkError("timeout".into()).is_retryable());
        assert!(PaymentError::RateLimited {
            provider: "stripe".into(),
            retry_after_secs: 60
        }
        .is_retryable());
        assert!(!PaymentError::InvalidRequest("bad data".into()).is_retryable());
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            PaymentError::InvalidRequest("test".into()).status_code(),
            400
        );
        assert_eq!(
            PaymentError::ProductNotFound {
                product_id: "x".into()
            }
            .status_code(),
            404
        );
        assert_eq!(
            PaymentError::RateLimited {
                provider: "stripe".into(),
                retry_after_secs: 60
            }
            .status_code(),
            429
        );
    }
}
