set dotenv-load

build: 
	@cargo build --verbose --all-features

test-user:
	@cargo test -p tradingview-rs --test user_test

quick-test:
	@cargo test

full-test: quick-test
	@cargo test --all-features -- --ignored

clippy:
	@cargo clippy --all-features --fix -- -D warnings

format:
	@cargo fmt --all -- --check

checks: build quick-test clippy format
	@git status

quote-example:
	cargo run --package tradingview-rs --example quote

chart-example:
	cargo run --package tradingview-rs --example chart

user-example:
	cargo run --package tradingview-rs --example user

client-example:
	cargo run --package tradingview-rs --example client

shared_session-example:
	cargo run --package tradingview-rs --example shared_session

lines-of-code:
	@git ls-files | grep '\.rs' | xargs wc -l