# Lightning-Cart (Rust)
Lighting speed and secure cart checkout payment engine in Rust, Rust Tokio Async using Strategy Design Pattern for Stripe Checkout API, Stripe Links API, Paypal and Square


## Features

- 🚀 **Lightning Fast** - Rust, Rust Tokio async for superior zero-cost overhead execution
- **Ultra Secure** - Server-side secrets, webhook signature verification, idempotency keys
- **Pluggable Providers** - Strategy pattern: Stripe (default), PayPal, Square (future)
- **Multiple Delivery Schemes** - Docker container, WASM bundle, SaaS API
- **Multiple Checkout Schemes** - Single-shot (one-time) payments or subscriptions
- **Production Grade** - Comprehensive error handling, logging, testing


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


## Cart Checkout Payment Strategy Trait

```rust
#[async_trait]
pub trait PaymentStrategy: Send + Sync {
    /// Create a checkout session, return redirect URL
    async fn create_checkout(
        &self,
        order: &Order,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSession, PaymentError>;

    /// Verify webhook signature, parse event
    async fn verify_webhook(
        &self,
        payload: &[u8],
        signature: &str,
    ) -> Result<WebhookEvent, PaymentError>;

    /// Provider name for logging/metrics
    fn provider_name(&self) -> &'static str;
}
```

## Quick Start

```bash
# Clone
git clone https://github.com/isgo-golgo13/lightning-cart-rs.git
cd lightning-cart-rs

# Configure
cp .env.example .env
# Edit .env with your Stripe keys

# Build
cargo build --release

# Run
cargo run --release -p pay-api
```

## Configuration

Create a `.env` file:

```env
# Stripe Configuration
STRIPE_SECRET_KEY=sk_test_...
STRIPE_PUBLISHABLE_KEY=pk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...

# Server Configuration
HOST=0.0.0.0
PORT=8080
BASE_URL=https://enginevector.io

# Environment
RUST_LOG=info,pay_api=debug
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/checkout` | Create checkout session |
| POST | `/webhook/stripe` | Stripe webhook handler |
| GET | `/health` | Health check |

### Create Checkout

```bash
curl -X POST http://localhost:8080/api/v1/checkout \
  -H "Content-Type: application/json" \
  -d '{
    "items": [
      {"product_id": "rang-play-rs-cli", "quantity": 1}
    ],
    "customer_email": "customer@example.com"
  }'
```

Response:
```json
{
  "session_id": "cs_test_...",
  "checkout_url": "https://checkout.stripe.com/c/pay/cs_test_...",
  "expires_at": "2025-01-02T12:00:00Z"
}
```

## Deployment Schemes

### Docker

```bash
docker build -t lightning-cart-rs .
docker run -p 8080:8080 --env-file .env lightning-cart-rs
```

### Fly.io (Recommended - Low Cost)

```bash
fly launch
fly secrets set STRIPE_SECRET_KEY=sk_live_...
fly deploy
```

### Cloudflare Workers (WASM)

```bash
cd crates/pay-wasm
wrangler publish
```

## Testing

```bash
# Unit tests
cargo test

# Integration test with Stripe CLI
stripe listen --forward-to localhost:8080/webhook/stripe
cargo test --features integration
```




## License

Proprietary - EngineVector.io