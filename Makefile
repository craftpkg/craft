.PHONY: build test lint fmt craft

all: build test lint

build:
	cargo build

test:
	cargo test

lint:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

craft:
	cargo run -p craft -- $(filter-out $@,$(MAKECMDGOALS))

test-pkg:
	@make craft add @tanstack/react-query react express -- --verbose
	@node main.js

%:
	@: