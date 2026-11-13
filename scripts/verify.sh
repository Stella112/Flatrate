#!/usr/bin/env bash
# make verify: everything a reviewer needs, with no wallet, no secret and no network writes.
#   1. shared/ unit and property tests
#   2. contract unit tests and contract builds (cargo near); the sandbox suite runs under `make test`
#   3. oracle: parsing tests against fixtures/prices, and the wasm run on those fixtures in
#      OutLayer's test runner (wasi-test) when it is available
#   4. deployed testnet contracts, view calls only (scripts/verify-chain.mjs)
#   5. a table: check, status, evidence; exit 1 on any FAIL
# Runs on Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build; use WSL).
set -uo pipefail
cd "$(dirname "$0")/.."
START=$(date +%s)
LOG=target/verify
mkdir -p "$LOG"
ROWS=()
row() { ROWS+=("$1"$'\t'"$2"$'\t'"$3"); printf '  %-44s %-7s %s\n' "$1" "$2" "$3"; }
last() { grep -E "$2" "$1" | tail -1; }

step() { # name, log file, command...
  local name="$1" log="$2"; shift 2
  echo "== $name"
  if "$@" > "$log" 2>&1; then return 0; else return 1; fi
}

# 1. shared
if step "shared tests" "$LOG/shared.log" cargo test -p flatrate-shared; then
  row "shared payoff/median/auction tests" PASS "$(grep -m1 'test result' "$LOG/shared.log")"
else row "shared payoff/median/auction tests" FAIL "see $LOG/shared.log"; fi

# 2. contracts
if step "contract unit tests" "$LOG/unit.log" bash -c 'cargo test -p fusd --lib && cargo test -p flatrate --lib'; then
  row "contract unit tests" PASS "$(grep -c '^test .* ok$' "$LOG/unit.log") passed"
else row "contract unit tests" FAIL "see $LOG/unit.log"; fi

if step "cargo near build" "$LOG/build.log" make build; then
  row "cargo near build (fusd, flatrate, mock)" PASS "$(ls target/wasm/*.wasm | xargs -n1 basename | tr '\n' ' ')"
else
  row "cargo near build (fusd, flatrate, mock)" FAIL "see $LOG/build.log"
fi
# The sandbox suite (29 tests, several minutes) runs under `make test`, not here
# (docs/Flatrate-full-description-v2.md, "Proof layer"; Maris's decision 2026-11-13).
row "sandbox tests" INFO "run separately: make test (or make demo-scripted for the recorded market)"

# 3. oracle
if step "oracle parsing tests" "$LOG/oracle.log" bash -c 'cd oracle && cargo test'; then
  row "oracle parsing vs fixtures/prices" PASS "$(grep -m1 'test result' "$LOG/oracle.log")"
else row "oracle parsing vs fixtures/prices" FAIL "see $LOG/oracle.log"; fi

ORACLE_HASH=""
ORACLE_WASM=""
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 && [ -f vendor/near-outlayer/scripts/build_github_wasm.sh ]; then
  echo "== oracle reproducible build (docker)"
  if bash scripts/oracle-reproducible.sh --keep > "$LOG/oracle-repro.log" 2>&1; then
    ORACLE_HASH=$(tail -1 "$LOG/oracle-repro.log" | grep -oE '^[0-9a-f]{64}$' || grep -oE '[0-9a-f]{64}' "$LOG/oracle-repro.log" | tail -1)
    ORACLE_WASM=$(grep -oE '/[^ ]+/module\.wasm' "$LOG/oracle-repro.log" | head -1)
    row "oracle reproducible build" PASS "sha256 $ORACLE_HASH"
  else row "oracle reproducible build" FAIL "see $LOG/oracle-repro.log"; fi
else
  row "oracle reproducible build" SKIP "needs docker and vendor/near-outlayer (see README)"
fi

RUNNER="${WASI_TEST:-$(command -v wasi-test || echo vendor/near-outlayer/wasi-examples/wasi-test-runner/target/release/wasi-test)}"
if [ -z "$ORACLE_WASM" ] && rustup target list --installed 2>/dev/null | grep -q wasm32-wasip2; then
  (cd oracle && cargo build --target wasm32-wasip2 --release > "../$LOG/oracle-wasm.log" 2>&1) && ORACLE_WASM=oracle/target/wasm32-wasip2/release/flatrate-oracle.wasm
fi
if [ -x "$RUNNER" ] && [ -n "$ORACLE_WASM" ]; then
  node -e '
    const fs=require("fs"); const src=["akash","vast","runpod","shadeform","verda"];
    fs.writeFileSync(process.argv[1], JSON.stringify({index:"H100_HOURLY",sources:src,fixture_responses:Object.fromEntries(src.map(s=>[s,fs.readFileSync(`fixtures/prices/${s}.json`,"utf8")]))}));
  ' "$LOG/oracle-fixture-input.json"
  "$RUNNER" --wasm "$ORACLE_WASM" --input-file "$LOG/oracle-fixture-input.json" > "$LOG/oracle-run.log" 2>&1
  if grep -q '"price_micros":3980000,"sources"' "$LOG/oracle-run.log" && grep -q "All checks passed" "$LOG/oracle-run.log"; then
    row "oracle wasm on fixtures (OutLayer runner)" PASS "median 3980000 = median(2.56, 3.976, 3.99, 3.98, 5.087)"
  else row "oracle wasm on fixtures (OutLayer runner)" FAIL "see $LOG/oracle-run.log"; fi
else
  row "oracle wasm on fixtures (OutLayer runner)" SKIP "build wasi-test: make oracle-runner (OutLayer wasi-test-runner)"
fi

# 4. chain
echo "== testnet view calls"
node scripts/verify-chain.mjs ${ORACLE_HASH:+--oracle-hash "$ORACLE_HASH"} > "$LOG/chain.log" 2>&1
while IFS=$'\t' read -r c s e; do [ -n "$c" ] && row "$c" "$s" "$e"; done < "$LOG/chain.log"

# 5. table
echo
echo "FLATRATE VERIFY  ($(($(date +%s) - START)) s, logs in $LOG/)"
printf '%-46s %-8s %s\n' CHECK STATUS EVIDENCE
for r in "${ROWS[@]}"; do IFS=$'\t' read -r c s e <<<"$r"; printf '%-46s %-8s %s\n' "$c" "$s" "$e"; done
FAILS=$(printf '%s\n' "${ROWS[@]}" | awk -F'\t' '$2=="FAIL"' | wc -l | tr -d ' ')
echo
if [ "$FAILS" = 0 ]; then echo "RESULT: no FAIL"; exit 0; else echo "RESULT: $FAILS FAIL"; exit 1; fi
