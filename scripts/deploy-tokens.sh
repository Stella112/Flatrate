#!/usr/bin/env bash
# Extra 2: deploys the same Flatrate contract a second time, for the LLM token-price index.
# The H100 contract (flatrate.flatrate-maris.testnet) is keyless and unchanged.
#
# Usage: WASM=target/wasm/flatrate.wasm ORACLE_VERSION=<sha256> bash scripts/deploy-tokens.sh
# Needs: near (near-cli-rs) with the root account's key in ~/.near-credentials (legacy keychain).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT_ID="${ROOT_ID:-flatrate-maris.testnet}"
TOKENS_ID="${TOKENS_ID:-tokens.$ROOT_ID}"
WASM="${WASM:?set WASM to the built flatrate.wasm}"
ORACLE_VERSION="${ORACLE_VERSION:?set ORACLE_VERSION to the token oracle sha256}"
FUNDING="${FUNDING:-8 NEAR}"

INIT=$(cat <<JSON
{
  "owner_id": "$ROOT_ID",
  "fusd_id": "fusd.$ROOT_ID",
  "outlayer_id": "outlayer.testnet",
  "oracle_project_id": "$ROOT_ID/flatrate-oracle-tokens",
  "oracle_version_key": "$ORACLE_VERSION",
  "oracle_sources": ["deepinfra", "akashml", "parasail", "groq", "sambanova", "coreweave", "google"],
  "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
  "demo_template": {"commit_ns": "180000000000", "reveal_ns": "120000000000", "period_ns": "480000000000", "slot_ns": "60000000000", "min_filled_slots": 3, "void_after_ns": "600000000000"},
  "weekly_template": {"commit_ns": "86400000000000", "reveal_ns": "43200000000000", "period_ns": "604800000000000", "slot_ns": "21600000000000", "min_filled_slots": 14, "void_after_ns": "604800000000000"},
  "observation_deposit": "6000000000000000000000",
  "index": "TOKEN_PRICE"
}
JSON
)
strip() { sed 's/\x1b\[[0-9;]*m//g'; }
tx() { grep -oE 'Transaction ID: \w+|ERR_\w+|rror.*' | head -2 | tr '\n' ' '; echo; }

echo "create $TOKENS_ID ($FUNDING)"
near account create-account fund-myself "$TOKENS_ID" "$FUNDING" autogenerate-new-keypair save-to-legacy-keychain \
  sign-as "$ROOT_ID" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
echo "deploy + new"
near contract deploy "$TOKENS_ID" use-file "$WASM" with-init-call new json-args "$INIT" prepaid-gas '100 Tgas' \
  attached-deposit '0 NEAR' network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
echo "fUSD storage for the contract"
near contract call-function as-transaction "fusd.$ROOT_ID" storage_deposit json-args "{\"account_id\":\"$TOKENS_ID\"}" \
  prepaid-gas '30 Tgas' attached-deposit '0.00125 NEAR' sign-as "$ROOT_ID" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
