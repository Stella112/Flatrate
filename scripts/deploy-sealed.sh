#!/usr/bin/env bash
# Extra (private auction): deploys the Flatrate contract a third time, for H100 markets whose
# opening auction is sealed to an OutLayer enclave and cleared inside it. The H100 contract
# (flatrate.flatrate-maris.testnet) is keyless and unchanged; this deployment keeps its
# full-access key (README Limitations).
#
# Usage: WASM=target/wasm/flatrate.wasm ENCLAVE_VERSION=<sha256 of auction-enclave wasm> bash scripts/deploy-sealed.sh
# Needs: near (near-cli-rs) with the root account's key in ~/.near-credentials (legacy keychain).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT_ID="${ROOT_ID:-flatrate-maris.testnet}"
SEALED_ID="${SEALED_ID:-sealed.$ROOT_ID}"
WASM="${WASM:?set WASM to the built flatrate.wasm}"
ORACLE_VERSION="${ORACLE_VERSION:-6c252d52804cb3f356ab1f5718069a57ab2d2fb96ad14e21eb18277fbcdaaf44}"
ENCLAVE_VERSION="${ENCLAVE_VERSION:?set ENCLAVE_VERSION to the auction-enclave wasm sha256}"
FUNDING="${FUNDING:-6.5 NEAR}"

INIT=$(cat <<JSON
{
  "owner_id": "$ROOT_ID",
  "fusd_id": "fusd.$ROOT_ID",
  "outlayer_id": "outlayer.testnet",
  "oracle_project_id": "$ROOT_ID/flatrate-oracle",
  "oracle_version_key": "$ORACLE_VERSION",
  "oracle_sources": ["akash", "vast", "runpod", "shadeform", "verda"],
  "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
  "demo_template": {"commit_ns": "240000000000", "reveal_ns": "60000000000", "period_ns": "480000000000", "slot_ns": "60000000000", "min_filled_slots": 3, "void_after_ns": "600000000000"},
  "weekly_template": {"commit_ns": "86400000000000", "reveal_ns": "43200000000000", "period_ns": "604800000000000", "slot_ns": "21600000000000", "min_filled_slots": 14, "void_after_ns": "604800000000000"},
  "observation_deposit": "6000000000000000000000",
  "index": "H100_HOURLY",
  "enclave": {"project_id": "$ROOT_ID/flatrate-auction", "version_key": "$ENCLAVE_VERSION"}
}
JSON
)
strip() { sed 's/\x1b\[[0-9;]*m//g'; }
tx() { grep -oE 'Transaction ID: \w+|ERR_\w+|rror.*' | head -2 | tr '\n' ' '; echo; }

echo "create $SEALED_ID ($FUNDING)"
near account create-account fund-myself "$SEALED_ID" "$FUNDING" autogenerate-new-keypair save-to-legacy-keychain \
  sign-as "$ROOT_ID" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
echo "deploy + new"
near contract deploy "$SEALED_ID" use-file "$WASM" with-init-call new json-args "$INIT" prepaid-gas '100 Tgas' \
  attached-deposit '0 NEAR' network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
echo "fUSD storage for the contract"
near contract call-function as-transaction "fusd.$ROOT_ID" storage_deposit json-args "{\"account_id\":\"$SEALED_ID\"}" \
  prepaid-gas '30 Tgas' attached-deposit '0.00125 NEAR' sign-as "$ROOT_ID" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | tx
echo "enclave key (OutLayer run through this contract)"
near contract call-function as-transaction "$SEALED_ID" request_enclave_key json-args '{}' \
  prepaid-gas '300 Tgas' attached-deposit '0 NEAR' sign-as "$ROOT_ID" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip | grep -oE 'Transaction ID: \w+|enclave_key_set.*|enclave_rejected.*|ERR_\w+' | head -3
