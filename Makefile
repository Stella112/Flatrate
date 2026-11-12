# Flatrate. Targets run on Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build;
# on Windows run these inside WSL).
SHELL := /bin/bash
WASM_DIR := target/wasm
CONTRACTS := fusd mock-outlayer flatrate

.PHONY: build test test-shared test-unit test-sandbox verify clean-wasm

## Build all contract wasm files into target/wasm with cargo-near.
build:
	@mkdir -p $(WASM_DIR)
	@for c in $(CONTRACTS); do \
	  echo "== cargo near build $$c"; \
	  cargo near build non-reproducible-wasm --manifest-path contracts/$$c/Cargo.toml --out-dir $(WASM_DIR) || exit 1; \
	done
	@ls -l $(WASM_DIR)/*.wasm

## shared/ unit and property tests.
test-shared:
	cargo test -p flatrate-shared

## Contract unit tests (near-sdk unit-testing).
test-unit:
	cargo test -p fusd --lib
	cargo test -p flatrate --lib

## near-workspaces sandbox tests (needs `make build` first).
test-sandbox: build
	cargo test -p flatrate --test sandbox -- --test-threads=1

## Everything that runs locally without network writes or keys.
test: test-shared test-unit test-sandbox

clean-wasm:
	rm -rf $(WASM_DIR)
