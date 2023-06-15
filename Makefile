#!make
include .env
export $(shell sed 's/=.*//' .env)

.PHONY: build test clippy format checks pipeline

build: 
	@cargo build --verbose --all-features

test:
	@cargo test --verbose --all-features

clippy:
	@cargo clippy --verbose --all-features

format:
	@cargo fmt --all -- --check

checks: build test clippy format
	@echo "### Don't forget to add untracked files! ###"
	@git status
	@echo "### Awesome work! 😍 ###"""

pipeline: build test clippy format
	@echo "### Don't forget to add untracked files! ###"
	@git status
	@echo "### Awesome work! 😍 ###"""

lib:
	@cargo run --bin datafeed

keygen:
	@tests/socketio-server/keygen.sh node-engine-io-secure 127.0.0.1
	