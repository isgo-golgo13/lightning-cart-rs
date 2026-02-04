# =============================================================================
# Lightning-Cart RS - Makefile
# =============================================================================

.PHONY: help build run test clean docker docker-flyio stripe-listen

# Default target
help:
	@echo "⚡ Lightning-Cart RS"
	@echo ""
	@echo "Usage:"
	@echo "  make build          Build release binary"
	@echo "  make run            Run development server"
	@echo "  make test           Run all tests"
	@echo "  make clean          Clean build artifacts"
	@echo "  make docker         Build Docker image (native)"
	@echo "  make docker-flyio   Build Docker image (linux/amd64 for Fly.io)"
	@echo "  make docker-run     Run Docker container"
	@echo "  make stripe-listen  Start Stripe webhook listener"
	@echo "  make fmt            Format code"
	@echo "  make lint           Run clippy linter"
	@echo ""

# Build release binary
build:
	cargo build --release -p pay-api

# Run development server
run:
	RUST_LOG=debug cargo run -p pay-api

# Run all tests
test:
	cargo test --workspace

# Run tests with coverage (requires cargo-tarpaulin)
coverage:
	cargo tarpaulin --workspace --out Html

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Format code
fmt:
	cargo fmt --all

# Run clippy linter
lint:
	cargo clippy --workspace -- -D warnings

# Build Docker image
docker:
	docker build -t lightning-cart-rs .

# Build Docker image for Fly.io (linux/amd64 from Apple Silicon)
docker-flyio:
	docker buildx build --platform linux/amd64 -t lightning-cart-rs:fly .

# Run Docker container
docker-run:
	docker run -p 8080:8080 --env-file .env lightning-cart-rs

# Start Docker Compose
up:
	docker-compose up

# Stop Docker Compose
down:
	docker-compose down

# Start Stripe webhook listener (requires Stripe CLI)
stripe-listen:
	@echo "Starting Stripe webhook listener..."
	@echo "Make sure you have Stripe CLI installed: brew install stripe/stripe-cli/stripe"
	stripe listen --forward-to localhost:8080/webhook/stripe

# Build WASM package (requires wasm-pack)
wasm:
	cd crates/pay-wasm && wasm-pack build --target web

# Install development dependencies
dev-deps:
	cargo install cargo-watch cargo-tarpaulin wasm-pack
	@echo "Install Stripe CLI: brew install stripe/stripe-cli/stripe"

# Watch for changes and rebuild (requires cargo-watch)
watch:
	cargo watch -x 'run -p pay-api'

# Check if environment is configured
check-env:
	@test -f .env || (echo "❌ .env file not found. Copy .env.example to .env" && exit 1)
	@grep -q "STRIPE_SECRET_KEY=sk_" .env || (echo "❌ STRIPE_SECRET_KEY not configured" && exit 1)
	@echo "Environment configured"
