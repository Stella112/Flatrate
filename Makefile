# Flatrate. Targets run on Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build;
# on Windows run these inside WSL).
SHELL := /bin/bash
WASM_DIR := target/wasm
CONTRACTS := fusd mock-outlayer flatrate

.PHONY: build test test-shared test-unit test-oracle test-sandbox oracle-wasm oracle-repro oracle-runner vendor verify demo-scripted keeper clean-wasm

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

## Oracle parsing and median against fixtures/prices (native, no network).
test-oracle:
	cd oracle && cargo test

## Oracle component for OutLayer (wasm32-wasip2).
oracle-wasm:
	cd oracle && cargo build --target wasm32-wasip2 --release
	@sha256sum oracle/target/wasm32-wasip2/release/flatrate-oracle.wasm

## near-workspaces sandbox tests (needs `make build` first).
test-sandbox: build
	cargo test -p flatrate --test sandbox -- --test-threads=1 --nocapture

## Everything that runs locally without network writes or keys.
test: test-shared test-unit test-oracle test-sandbox

## OutLayer sources (build script and test runner), pinned to the commit read in Phase 0.
vendor:
	@test -d vendor/near-outlayer || git clone -q https://github.com/fastnear/near-outlayer vendor/near-outlayer
	git -C vendor/near-outlayer checkout -q 2d50bfab14389857e6e50e84e76d5652b944e1cc

## Reproducible oracle build in OutLayer's pinned compiler image (needs docker); prints the sha256.
oracle-repro: vendor
	bash scripts/oracle-reproducible.sh --twice

## OutLayer's local WASI test runner (wasi-test), used by make verify. Takes several minutes once.
oracle-runner: vendor
	cd vendor/near-outlayer/wasi-examples/wasi-test-runner && cargo build --release

## Reviewer checks in a few minutes: shared tests, builds, oracle, read-only testnet checks, a table.
## The sandbox suite (slower) runs under `make test`.
verify:
	bash scripts/verify.sh

## Replays testnet market 2 from fixtures/demo/market-2.json in the sandbox (no keys, no network).
demo-scripted: build
	cargo test -p flatrate --test sandbox scripted_demo_replays_market_2 -- --nocapture

## One keeper pass: KEEPER_ACCOUNT=<you>.testnet make keeper
keeper:
	node scripts/keeper.mjs --once

clean-wasm:
	rm -rf $(WASM_DIR)
