//! # pay-wasm
//!
//! WebAssembly bindings for lightning-cart-rs.
//!
//! This crate provides WASM-compatible functions for:
//! - Creating checkout sessions from browser/edge
//! - Validating cart data client-side
//! - Price calculations
//!
//! ## Usage (JavaScript)
//!
//! ```javascript
//! import init, { create_order, calculate_total } from 'lightning-cart-wasm';
//!
//! await init();
//!
//! const order = create_order([
//!   { product_id: 'rang-play-rs-cli', quantity: 1 }
//! ]);
//!
//! console.log('Total:', calculate_total(order));
//! ```
//!
//! ## Building
//!
//! ```bash
//! wasm-pack build --target web
//! ```

use pay_core::{Currency, LineItem, Order, Price, Product};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Initialize the WASM module (called automatically)
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Cart item for WASM interface
#[derive(Debug, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct WasmCartItem {
    product_id: String,
    name: String,
    price_cents: i64,
    quantity: u32,
}

#[wasm_bindgen]
impl WasmCartItem {
    #[wasm_bindgen(constructor)]
    pub fn new(product_id: String, name: String, price_cents: i64, quantity: u32) -> Self {
        Self {
            product_id,
            name,
            price_cents,
            quantity,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn product_id(&self) -> String {
        self.product_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn price_cents(&self) -> i64 {
        self.price_cents
    }

    #[wasm_bindgen(getter)]
    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    /// Calculate line item total in cents
    #[wasm_bindgen]
    pub fn total_cents(&self) -> i64 {
        self.price_cents * self.quantity as i64
    }

    /// Format price for display
    #[wasm_bindgen]
    pub fn format_price(&self) -> String {
        format!("${:.2}", self.price_cents as f64 / 100.0)
    }

    /// Format total for display
    #[wasm_bindgen]
    pub fn format_total(&self) -> String {
        format!("${:.2}", self.total_cents() as f64 / 100.0)
    }
}

/// Calculate total for a list of cart items
#[wasm_bindgen]
pub fn calculate_cart_total(items: JsValue) -> Result<i64, JsValue> {
    let items: Vec<WasmCartItem> = serde_wasm_bindgen::from_value(items)
        .map_err(|e| JsValue::from_str(&format!("Invalid cart items: {}", e)))?;

    let total: i64 = items.iter().map(|item| item.total_cents()).sum();
    Ok(total)
}

/// Format a price in cents to display string
#[wasm_bindgen]
pub fn format_price(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

/// Validate a product ID format
#[wasm_bindgen]
pub fn validate_product_id(product_id: &str) -> bool {
    !product_id.is_empty()
        && product_id.len() <= 100
        && product_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Log to browser console
#[wasm_bindgen]
pub fn log(message: &str) {
    web_sys::console::log_1(&JsValue::from_str(message));
}

/// Get library version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cart_item_total() {
        let item = WasmCartItem::new(
            "test".to_string(),
            "Test Product".to_string(),
            1999,
            2,
        );
        assert_eq!(item.total_cents(), 3998);
    }

    #[test]
    fn test_format_price() {
        assert_eq!(format_price(1999), "$19.99");
        assert_eq!(format_price(100), "$1.00");
    }

    #[test]
    fn test_validate_product_id() {
        assert!(validate_product_id("rang-play-rs"));
        assert!(validate_product_id("product_123"));
        assert!(!validate_product_id(""));
        assert!(!validate_product_id("invalid id"));
    }
}
