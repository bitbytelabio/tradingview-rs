set dotenv-load

build: 
	@cargo build --verbose --all-features

test-user:
	@cargo test -p tradingview-rs --test user_test

test-all:
	@cargo test --verbose --all-features

clippy:
	@cargo clippy --verbose --all-features

format:
	@cargo fmt --all -- --check

checks: build test-all clippy format
	@echo "### Don't forget to add untracked files! ###"
	@git status
	@echo "### Awesome work! 😍 ###"""

examples:
	cargo run --package datafeed --example auth