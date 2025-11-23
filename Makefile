.PHONY: build test lint craft

all: build test lint

build:
	cargo build

test:
	cargo test

lint:
	cargo clippy --workspace -- -D warnings

craft:
	cargo run -p craft -- $(filter-out $@,$(MAKECMDGOALS))

test-pkg:
	@make craft add @tanstack/react-query react express -- --verbose
	@node main.js

%:
	@: