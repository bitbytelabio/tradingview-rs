set dotenv-load

build: 
	@cargo build --verbose --all-features

test-user:
	@cargo test -p tradingview-rs --test user_test

test-all:
	@cargo test --all-features

clippy:
	@cargo clippy --all-features --fix -- -D warnings

format:
	@cargo fmt --all -- --check

checks: build test-all clippy format
	@git status

quote-example:
	cargo run --package tradingview-rs --example quote

user-example:
	cargo run --package tradingview-rs --example user

client-example:
	cargo run --package tradingview-rs --example client