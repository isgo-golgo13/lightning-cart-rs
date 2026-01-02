//! # Application State
//!
//! Shared state for the Axum application.
//! Contains payment strategies, configuration, and product catalog.

use pay_core::{BoxedPaymentStrategy, CheckoutUrls, PaymentStrategySelector, ProductCatalog};
use pay_stripe::StripeCheckoutStrategy;
use std::sync::Arc;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Host to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// Base URL for callbacks
    pub base_url: String,
    /// Environment (development, staging, production)
    pub environment: String,
}

impl AppConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            base_url: std::env::var("BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        }
    }

    /// Get the socket address to bind to
    pub fn socket_addr(&self) -> std::net::SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .expect("Invalid socket address")
    }

    /// Check if running in production
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Payment strategy selector
    pub strategies: PaymentStrategySelector,
    /// Product catalog
    pub catalog: ProductCatalog,
    /// Checkout URLs
    pub urls: CheckoutUrls,
    /// Application config
    pub config: AppConfig,
}

impl AppState {
    /// Create a new AppState with default Stripe strategy
    pub fn new() -> anyhow::Result<Self> {
        let config = AppConfig::from_env();
        let urls = CheckoutUrls::new(&config.base_url);

        // Load product catalog
        let catalog = load_product_catalog()?;

        // Initialize payment strategies
        let stripe_strategy = StripeCheckoutStrategy::from_env()
            .map_err(|e| anyhow::anyhow!("Failed to initialize Stripe: {}", e))?;

        let mut strategies = PaymentStrategySelector::new("stripe");
        strategies.register(Arc::new(stripe_strategy) as BoxedPaymentStrategy);

        Ok(Self {
            strategies,
            catalog,
            urls,
            config,
        })
    }

    /// Get the default payment strategy
    pub fn default_strategy(&self) -> Option<&BoxedPaymentStrategy> {
        self.strategies.default_strategy()
    }

    /// Get a specific payment strategy
    pub fn strategy(&self, provider: &str) -> Option<&BoxedPaymentStrategy> {
        self.strategies.get(provider)
    }

    /// Get success URL with session ID placeholder
    pub fn success_url(&self) -> String {
        format!("{}?session_id={{CHECKOUT_SESSION_ID}}", self.urls.success_url())
    }

    /// Get cancel URL
    pub fn cancel_url(&self) -> String {
        self.urls.cancel_url()
    }
}

/// Load product catalog from config file
fn load_product_catalog() -> anyhow::Result<ProductCatalog> {
    // Try to load from config/products.toml
    let config_paths = [
        "config/products.toml",
        "../config/products.toml",
        "../../config/products.toml",
    ];

    for path in config_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let catalog: ProductCatalog = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path, e))?;
            tracing::info!("Loaded {} products from {}", catalog.products.len(), path);
            return Ok(catalog);
        }
    }

    // Return empty catalog if no config found
    tracing::warn!("No product catalog found, using empty catalog");
    Ok(ProductCatalog::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_defaults() {
        // Clear env vars for test
        std::env::remove_var("HOST");
        std::env::remove_var("PORT");
        std::env::remove_var("BASE_URL");

        let config = AppConfig::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_socket_addr() {
        let config = AppConfig {
            host: "0.0.0.0".to_string(),
            port: 3000,
            base_url: "http://localhost:3000".to_string(),
            environment: "test".to_string(),
        };

        let addr = config.socket_addr();
        assert_eq!(addr.to_string(), "0.0.0.0:3000");
    }
}
