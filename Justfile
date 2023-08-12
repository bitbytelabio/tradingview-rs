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

chart-example:
	cargo run --package tradingview-rs --example chart

user-example:
	cargo run --package tradingview-rs --example user

client-example:
	cargo run --package tradingview-rs --example client

session_clone-example:
	cargo run --package tradingview-rs --example session_clone

lines-of-code:
	@git ls-files | grep '\.rs' | xargs wc -l