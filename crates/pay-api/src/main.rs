//! # Lightning-Cart RS
//!
//! Ultra-fast, ultra-secure payment engine.
//!
//! ## Usage
//!
//! ```bash
//! # Set environment variables
//! export STRIPE_SECRET_KEY=sk_test_...
//! export STRIPE_PUBLISHABLE_KEY=pk_test_...
//! export STRIPE_WEBHOOK_SECRET=whsec_...
//!
//! # Run the server
//! lightning-cart
//! ```

use pay_api::{routes, state::AppState};
use tracing::{info, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    // Print banner
    print_banner();

    // Initialize application state
    let state = AppState::new()?;

    let addr = state.config.socket_addr();
    let is_prod = state.config.is_production();

    info!("Environment: {}", state.config.environment);
    info!("Products loaded: {}", state.catalog.products.len());
    info!(
        "Payment providers: {:?}",
        state.strategies.providers()
    );

    // Create router
    let app = routes::create_router(state);

    // Start server
    info!("🚀 Lightning-Cart starting on http://{}", addr);

    if !is_prod {
        info!("📝 API docs: http://{}/health", addr);
        info!("💳 Checkout: POST http://{}/api/v1/checkout", addr);
        info!("🔔 Webhook: POST http://{}/webhook/stripe", addr);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn print_banner() {
    println!(
        r#"
  ⚡ Lightning-Cart RS ⚡
  ━━━━━━━━━━━━━━━━━━━━━━━
  Ultra-fast payment engine
  Version: {}
  
"#,
        env!("CARGO_PKG_VERSION")
    );
}
