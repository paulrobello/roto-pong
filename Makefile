# Roto Pong Makefile

# Use cargo from standard locations
CARGO := $(or $(shell which cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
TRUNK := $(or $(shell which trunk 2>/dev/null),$(HOME)/.cargo/bin/trunk)
RUSTUP := $(or $(shell which rustup 2>/dev/null),$(HOME)/.cargo/bin/rustup)

.PHONY: all build build-release check test run serve clean fmt lint fix wasm wasm-release install-deps

# Default target
all: check test

# Install required tools
install-deps:
	$(RUSTUP) target add wasm32-unknown-unknown
	$(CARGO) install trunk
	$(CARGO) install wasm-opt

# Check compilation (fast)
check:
	$(CARGO) check

# Check WASM compilation
check-wasm:
	$(CARGO) check --target wasm32-unknown-unknown

# Build debug
build:
	$(CARGO) build

# Build release
build-release:
	$(CARGO) build --release

# Run tests
test:
	$(CARGO) test

# Run tests with output
test-verbose:
	$(CARGO) test -- --nocapture

# Run native binary (limited functionality)
run:
	$(CARGO) run

# Serve WASM build with hot reload
serve:
	$(TRUNK) serve

# Serve on all interfaces (for mobile testing)
serve-public:
	$(TRUNK) serve --address 0.0.0.0

# Build WASM (debug)
wasm:
	$(TRUNK) build

# Build WASM (release, optimized)
wasm-release:
	$(TRUNK) build --release

# Format code
fmt:
	$(CARGO) fmt

# Check formatting
fmt-check:
	$(CARGO) fmt -- --check

# Run clippy lints
lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

# Lint WASM target
lint-wasm:
	$(CARGO) clippy --target wasm32-unknown-unknown -- -D warnings

# Auto-fix lints where possible
fix:
	$(CARGO) clippy --fix --allow-dirty

# Clean build artifacts
clean:
	$(CARGO) clean
	rm -rf dist/

# Full CI check (what CI runs)
ci: fmt-check lint test check-wasm

# Watch for changes and rebuild
watch:
	$(CARGO) watch -x check

# Generate docs
docs:
	$(CARGO) doc --open

# Show project stats
stats:
	@echo "Lines of code:"
	@find src -name '*.rs' | xargs wc -l | tail -1
	@echo "\nTest count:"
	@$(CARGO) test -- --list 2>/dev/null | grep -c "test$$" || echo "Run 'make test' first"
