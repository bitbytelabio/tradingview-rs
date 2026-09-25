set dotenv-load

default: checks

# Build the entire workspace with all features
build:
	@cargo build --workspace --all-targets --all-features

# Run all Rust unit and integration tests across the workspace
test:
	@cargo test --workspace --all-features --locked

# Run user authentication, session, and CAPTCHA tests
test-user:
	@cargo test -p tradingview-rs --locked --lib user:: --test user_test

# Run Python integration test suite
test-python:
	@cd crates/tradingview-py && if [ -f .venv/bin/pytest ]; then .venv/bin/pytest -v; else pytest -v; fi

# Lint and typecheck Python bindings with Ruff and Mypy
lint-python:
	@cd crates/tradingview-py && \
		if [ -f .venv/bin/ruff ]; then .venv/bin/ruff check python tests && .venv/bin/ruff format --check python tests; else ruff check python tests && ruff format --check python tests; fi && \
		if [ -f .venv/bin/mypy ]; then .venv/bin/mypy python/ tests/; else mypy python/ tests/; fi

# Build and install Python PyO3 native extension in editable mode
build-python:
	@cd crates/tradingview-py && if [ -f .venv/bin/maturin ]; then .venv/bin/maturin develop --release; else maturin develop --release; fi

# Run full test suite including network/auth ignored tests
full-test: test
	@cargo test --workspace --all-features -- --ignored

# Run Criterion microbenchmarks
bench:
	@cargo bench --all-features

# Run Clippy linter across workspace in strict mode
clippy:
	@cargo clippy --workspace --all-targets --all-features -- -D warnings

# Apply automatic Clippy fixes
clippy-fix:
	@cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

# Check code formatting with rustfmt
format-check:
	@cargo fmt --all -- --check

# Automatically format all Rust source files
format:
	@cargo fmt --all

# Verify crate documentation with zero warnings
docs:
	@cargo doc --no-deps --all-features --locked

# Complete developer verification gate across Rust and Python
checks: format-check clippy test docs lint-python test-python
	@git status --short

# Run a specific example binary
example bin:
	cargo run --package tradingview-rs --example {{bin}}

# Count lines of code
lines-of-code:
	@cloc $(git ls-files)

# Scan repository for secrets using GitGuardian
creds-scan:
	@ggshield secret scan repo ./

# Check for unused dependencies with nightly cargo udeps
udeps:
	@cargo +nightly udeps --all-targets --all-features -- -D warnings

# Clean build artifacts
clean:
	@cargo clean