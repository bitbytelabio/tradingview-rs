set dotenv-load

build: 
	@cargo build --verbose --all-features

test-user:
	@cargo test -p tradingview-rs --test user_test

quick-test:
	@cargo test --all-features

bench:
	@cargo bench --all-features

full-test: quick-test
	@cargo test --all-features -- --ignored

clippy:
	@cargo clippy --all-features --fix -- -D warnings

format:
	@cargo fmt --all -- --check

checks: build quick-test clippy format
	@git status

example bin:
	cargo run --package tradingview-rs --example {{bin}}

lines-of-code:
	@git ls-files | grep '\.rs' | xargs wc -l

creds-scan:
	@ggshield secret scan repo ./

udeps:
	@cargo +nightly udeps --all-targets --all-features -- -D warnings