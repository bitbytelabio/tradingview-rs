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
	
run-test-server:
	@docker build -t test_suite:latest -f tests/socketio-server/Dockerfile tests/socketio-server
	@docker run -d --name test_suite -p 4200:4200 -p 4201:4201 -p 4202:4202 -p 4203:4203 -p 4204:4204 -p 4205:4205 -p 4206:4206 test_suite:latest