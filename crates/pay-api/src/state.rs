//! # Application State
//!
//! Shared state for the Axum application.
//! Contains payment strategies, configuration, site registry, and product catalog.

use pay_core::{BoxedPaymentStrategy, CheckoutUrls, PaymentStrategySelector, ProductCatalog, Site, SiteRegistry};
use pay_stripe::StripeCheckoutStrategy;
use std::collections::HashMap;
use std::sync::Arc;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Host to bind to
    pub host: String,
    /// Port to listen on
    pub port: u16,
    /// Base URL for callbacks (fallback)
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
    /// Site registry (multi-tenant)
    pub sites: SiteRegistry,
    /// Checkout URLs (fallback for legacy routes)
    pub urls: CheckoutUrls,
    /// Application config
    pub config: AppConfig,
    /// HTTP client for webhook forwarding
    pub http_client: reqwest::Client,
    /// Webhook forward URLs per site (site_id → Vercel webhook URL)
    pub webhook_forward_urls: HashMap<String, String>,
}

impl AppState {
    /// Create a new AppState with default Stripe strategy
    pub fn new() -> anyhow::Result<Self> {
        let config = AppConfig::from_env();
        let urls = CheckoutUrls::new(&config.base_url);

        // Load product catalog
        let catalog = load_product_catalog()?;

        // Load site registry
        let sites = load_site_registry()?;

        // Initialize payment strategies
        let stripe_strategy = StripeCheckoutStrategy::from_env()
            .map_err(|e| anyhow::anyhow!("Failed to initialize Stripe: {}", e))?;

        let mut strategies = PaymentStrategySelector::new("stripe");
        strategies.register(Arc::new(stripe_strategy) as BoxedPaymentStrategy);

        // HTTP client for webhook forwarding to Vercel
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to create HTTP client");

        // Load webhook forward URLs from environment
        let mut webhook_forward_urls = HashMap::new();
        if let Ok(url) = std::env::var("CHARGEGUN_WEBHOOK_URL") {
            tracing::info!("Webhook forwarding enabled for chargegun → {}", url);
            webhook_forward_urls.insert("chargegun".to_string(), url);
        }
        if let Ok(url) = std::env::var("LUCKYDRONE_WEBHOOK_URL") {
            tracing::info!("Webhook forwarding enabled for luckydrone → {}", url);
            webhook_forward_urls.insert("luckydrone".to_string(), url);
        }
        if let Ok(url) = std::env::var("DRONEGRID_WEBHOOK_URL") {
            tracing::info!("Webhook forwarding enabled for dronegrid → {}", url);
            webhook_forward_urls.insert("dronegrid".to_string(), url);
        }
        if let Ok(url) = std::env::var("SPOKENHOPE_WEBHOOK_URL") {
            tracing::info!("Webhook forwarding enabled for spokenhope → {}", url);
            webhook_forward_urls.insert("spokenhope".to_string(), url);
        }

        Ok(Self {
            strategies,
            catalog,
            sites,
            urls,
            config,
            http_client,
            webhook_forward_urls,
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

    /// Get a site by ID, or default if not found
    pub fn get_site(&self, site_id: Option<&str>) -> Option<&Site> {
        self.sites.get_or_default(site_id)
    }

    /// Get success URL for a site (with session ID placeholder)
    pub fn success_url_for_site(&self, site_id: Option<&str>) -> String {
        if let Some(site) = self.get_site(site_id) {
            site.success_url_with_session()
        } else {
            // Fallback to default URLs
            format!("{}?session_id={{CHECKOUT_SESSION_ID}}", self.urls.success_url())
        }
    }

    /// Get cancel URL for a site
    pub fn cancel_url_for_site(&self, site_id: Option<&str>) -> String {
        if let Some(site) = self.get_site(site_id) {
            site.cancel_url.clone()
        } else {
            self.urls.cancel_url()
        }
    }

    /// Get statement descriptor suffix for a site
    pub fn statement_descriptor_for_site(&self, site_id: Option<&str>) -> Option<String> {
        self.get_site(site_id)
            .map(|s| s.statement_descriptor_suffix.clone())
            .filter(|s| !s.is_empty())
    }

    /// Get success URL with session ID placeholder (legacy, uses fallback)
    pub fn success_url(&self) -> String {
        format!("{}?session_id={{CHECKOUT_SESSION_ID}}", self.urls.success_url())
    }

    /// Get cancel URL (legacy, uses fallback)
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

/// Load site registry from config file
fn load_site_registry() -> anyhow::Result<SiteRegistry> {
    // Check for SITES_CONFIG env var override (e.g., config/sites-dev.toml for local testing)
    let env_path = std::env::var("SITES_CONFIG").ok();
    
    let config_paths: Vec<&str> = if let Some(ref p) = env_path {
        tracing::info!("Using SITES_CONFIG override: {}", p);
        vec![p.as_str()]
    } else {
        vec![
            "config/sites.toml",
            "../config/sites.toml",
            "../../config/sites.toml",
        ]
    };

    for path in config_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let mut registry: SiteRegistry = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path, e))?;
            
            // Set default site to chargegun
            registry.set_default("chargegun");
            
            tracing::info!("Loaded {} sites from {}", registry.len(), path);
            return Ok(registry);
        }
    }

    // Return default registry with chargegun site if no config found
    tracing::warn!("No site registry found, using default chargegun site");
    
    let mut registry = SiteRegistry::with_default("chargegun");
    registry.add(
        Site::new("chargegun", "ChargeGun", "chargegun.io")
            .with_statement_descriptor("CHARGEGUN")
            .with_success_url("https://chargegun.io/checkout/success")
            .with_cancel_url("https://chargegun.io/checkout/cancel")
            .with_support_email("info@chargegun.io")
    );
    
    Ok(registry)
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
