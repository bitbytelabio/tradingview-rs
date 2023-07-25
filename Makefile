#!make
-include .env

ifeq ($(wildcard .env),)
$(info .env file not found. Continuing without environment variables from .env file.)
else
export $(shell sed 's/=.*//' .env)
endif

.PHONY: build test clippy format checks pipeline

build: 
	@cargo build --verbose --all-features

test-user:
	@cargo test -p tradingview-rs --test user_test

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
	
examples:
	cargo run --package datafeed --example auth