//! # Product Types
//!
//! Product catalog types for lightning-cart.
//! Products are loaded from `config/products.toml`.

use serde::{Deserialize, Serialize};

/// Supported currencies (ISO 4217)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Currency {
    USD,
    EUR,
    GBP,
    JPY,
    CAD,
    AUD,
    CHF,
    MXN,
}

impl Currency {
    /// Returns the ISO 4217 currency code
    pub fn as_str(&self) -> &'static str {
        match self {
            Currency::USD => "usd",
            Currency::EUR => "eur",
            Currency::GBP => "gbp",
            Currency::JPY => "jpy",
            Currency::CAD => "cad",
            Currency::AUD => "aud",
            Currency::CHF => "chf",
            Currency::MXN => "mxn",
        }
    }

    /// Returns the number of decimal places for this currency
    /// (JPY has 0 decimals, most others have 2)
    pub fn decimal_places(&self) -> u8 {
        match self {
            Currency::JPY => 0,
            _ => 2,
        }
    }

    /// Convert a decimal amount to the smallest currency unit (cents, etc.)
    pub fn to_smallest_unit(&self, amount: f64) -> i64 {
        let multiplier = 10_f64.powi(self.decimal_places() as i32);
        (amount * multiplier).round() as i64
    }

    /// Convert from smallest unit back to decimal
    pub fn from_smallest_unit(&self, amount: i64) -> f64 {
        let divisor = 10_f64.powi(self.decimal_places() as i32);
        amount as f64 / divisor
    }
}

impl Default for Currency {
    fn default() -> Self {
        Currency::USD
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str().to_uppercase())
    }
}

/// Price with amount in smallest currency unit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price {
    /// Amount in smallest currency unit (cents for USD)
    pub amount: i64,
    /// Currency
    pub currency: Currency,
}

impl Price {
    /// Create a new price from decimal amount
    pub fn new(amount: f64, currency: Currency) -> Self {
        Self {
            amount: currency.to_smallest_unit(amount),
            currency,
        }
    }

    /// Create a price from smallest unit (cents)
    pub fn from_cents(amount: i64, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Get the decimal amount
    pub fn as_decimal(&self) -> f64 {
        self.currency.from_smallest_unit(self.amount)
    }

    /// Format for display (e.g., "$10.00")
    pub fn display(&self) -> String {
        let symbol = match self.currency {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
            Currency::JPY => "¥",
            Currency::CAD => "C$",
            Currency::AUD => "A$",
            Currency::CHF => "CHF ",
            Currency::MXN => "MX$",
        };
        if self.currency.decimal_places() == 0 {
            format!("{}{}", symbol, self.amount)
        } else {
            format!("{}{:.2}", symbol, self.as_decimal())
        }
    }
}

/// Billing interval for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BillingInterval {
    /// One-time payment (not a subscription)
    OneTime,
    /// Weekly billing
    Weekly,
    /// Monthly billing
    Monthly,
    /// Yearly billing
    Yearly,
}

impl Default for BillingInterval {
    fn default() -> Self {
        BillingInterval::OneTime
    }
}

/// Product type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    /// Digital download (WASM, Docker image, etc.)
    Digital,
    /// SaaS subscription
    Subscription,
    /// API access
    ApiAccess,
    /// Physical product (future)
    Physical,
}

impl Default for ProductType {
    fn default() -> Self {
        ProductType::Digital
    }
}

/// A product in the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    /// Unique product identifier (e.g., "rang-play-rs-pro")
    pub id: String,

    /// Display name
    pub name: String,

    /// Short description
    pub description: String,

    /// Product type
    #[serde(default)]
    pub product_type: ProductType,

    /// Price
    pub price: Price,

    /// Billing interval (for subscriptions)
    #[serde(default)]
    pub billing_interval: BillingInterval,

    /// Whether this product is active and available for purchase
    #[serde(default = "default_true")]
    pub active: bool,

    /// Optional image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,

    /// Optional metadata (license tier, features, etc.)
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Product {
    /// Create a new one-time purchase product
    pub fn one_time(id: impl Into<String>, name: impl Into<String>, price: Price) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            product_type: ProductType::Digital,
            price,
            billing_interval: BillingInterval::OneTime,
            active: true,
            image_url: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a new subscription product
    pub fn subscription(
        id: impl Into<String>,
        name: impl Into<String>,
        price: Price,
        interval: BillingInterval,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            product_type: ProductType::Subscription,
            price,
            billing_interval: interval,
            active: true,
            image_url: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Builder: set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Builder: set image URL
    pub fn with_image(mut self, url: impl Into<String>) -> Self {
        self.image_url = Some(url.into());
        self
    }

    /// Builder: add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if this is a subscription product
    pub fn is_subscription(&self) -> bool {
        !matches!(self.billing_interval, BillingInterval::OneTime)
    }
}

/// Product catalog (loaded from config)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProductCatalog {
    pub products: Vec<Product>,
}

impl ProductCatalog {
    /// Create an empty catalog
    pub fn new() -> Self {
        Self {
            products: Vec::new(),
        }
    }

    /// Add a product to the catalog
    pub fn add(&mut self, product: Product) {
        self.products.push(product);
    }

    /// Find a product by ID
    pub fn get(&self, id: &str) -> Option<&Product> {
        self.products.iter().find(|p| p.id == id)
    }

    /// Get all active products
    pub fn active_products(&self) -> impl Iterator<Item = &Product> {
        self.products.iter().filter(|p| p.active)
    }

    /// Load catalog from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_conversion() {
        let usd = Currency::USD;
        assert_eq!(usd.to_smallest_unit(10.99), 1099);
        assert_eq!(usd.from_smallest_unit(1099), 10.99);

        let jpy = Currency::JPY;
        assert_eq!(jpy.to_smallest_unit(1000.0), 1000);
        assert_eq!(jpy.from_smallest_unit(1000), 1000.0);
    }

    #[test]
    fn test_price_display() {
        let price = Price::new(29.99, Currency::USD);
        assert_eq!(price.display(), "$29.99");

        let price_eur = Price::new(19.99, Currency::EUR);
        assert_eq!(price_eur.display(), "€19.99");
    }

    #[test]
    fn test_product_builder() {
        let product = Product::one_time("test-product", "Test Product", Price::new(9.99, Currency::USD))
            .with_description("A test product")
            .with_metadata("tier", "pro");

        assert_eq!(product.id, "test-product");
        assert_eq!(product.description, "A test product");
        assert_eq!(product.metadata.get("tier"), Some(&"pro".to_string()));
        assert!(!product.is_subscription());
    }

    #[test]
    fn test_subscription_product() {
        let product = Product::subscription(
            "api-pro",
            "API Pro Plan",
            Price::new(29.0, Currency::USD),
            BillingInterval::Monthly,
        );

        assert!(product.is_subscription());
        assert_eq!(product.billing_interval, BillingInterval::Monthly);
    }
}
