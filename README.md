# Lightning-Cart (Rust)
Lighting speed and secure cart checkout payment engine in Rust, Rust Tokio Async using Strategy Design Pattern for Stripe Checkout API, Stripe Links API, Paypal and Square


## Project Structure

```shell
lighting-cart-rs/
├── Cargo.toml (workspace)
├── crates/
│   ├── pay-core/           # PaymentStrategy trait, Product, Order types
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── strategy.rs    # trait PaymentStrategy
│   │       ├── product.rs     # Product, Price, Currency
│   │       ├── order.rs       # Order, LineItem, CheckoutSession
│   │       └── error.rs       # PaymentError enum
│   │
│   ├── pay-stripe/         # StripeCheckoutStrategy, StripeLinksStrategy
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── checkout.rs    # Stripe Checkout Sessions
│   │       ├── links.rs       # Stripe Payment Links
│   │       ├── webhook.rs     # Signature verification
│   │       └── config.rs      # StripeConfig (keys from env)
│   │
│   ├── pay-api/            # Axum HTTP layer
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── routes.rs      # POST /checkout, POST /webhook
│   │       ├── handlers.rs    # Request handlers
│   │       └── state.rs       # AppState with strategy injection
│   │
│   └── pay-wasm/           # Optional: WASM for edge deployment
│
├── config/
│   └── products.toml       # Product catalog
│
└── templates/
    └── test-checkout/      # $10 Sabadell → FECU test
```