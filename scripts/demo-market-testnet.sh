#!/usr/bin/env bash
# Runs one Flatrate market on testnet from sealed bids to redemption, logging every tx id.
# The bids must already be committed (see PROGRESS.md Phase 3). Times come from the market.
#
# Usage: ACCOUNT=<you>.testnet MARKET=0 BIDS=target/demo-bids.txt bash scripts/demo-market-testnet.sh
# BIDS lines: "<label> side=<Long|Short> qty=<n> limit=<cents> salt=<salt> escrow=<micro>"
# Needs: near (near-cli-rs) with ACCOUNT's key in ~/.near-credentials, node (keeper), python3.
set -uo pipefail
PY=python3; python3 -c "" >/dev/null 2>&1 || PY=python
cd "$(dirname "$0")/.."
F="${FLATRATE_ID:-flatrate.flatrate-maris.testnet}"
A="${ACCOUNT:?set ACCOUNT}"
M="${MARKET:?set MARKET}"
BIDS="${BIDS:?set BIDS}"
OBS="${OBSERVATIONS:-4}"

log() { echo "$(date -u +%FT%TZ) $*"; }
strip() { sed 's/\x1b\[[0-9;]*m//g'; }
tx() { # method json-args [gas] [deposit]
  local out i
  for i in 1 2 3; do
    out=$(near contract call-function as-transaction "$F" "$1" json-args "$2" prepaid-gas "${3:-100 Tgas}" \
      attached-deposit "${4:-0 NEAR}" sign-as "$A" network-config testnet sign-with-legacy-keychain send 2>&1 | strip)
    # Retry only when nothing was sent (network error before a transaction id was printed).
    grep -q 'Transaction ID' <<<"$out" && break
    grep -q 'error sending request' <<<"$out" || break
    log "$1 network error, retry $i"; sleep 10
  done
  log "$1 tx=$(grep -oE 'Transaction ID: \w+' <<<"$out" | head -1 | cut -d' ' -f3)"
  grep -oE 'EVENT_JSON:.*' <<<"$out" | sed 's/^/    /'
  grep -iE 'panicked|Smart contract panicked|Error' <<<"$out" | head -3 | sed 's/^/    ERR /'
}
mfield() { # market field
  near contract call-function as-read-only "$F" get_market json-args "{\"market_id\":\"$M\"}" network-config testnet now 2>&1 \
    | strip | sed -n '/^{/,/^}/p' | $PY -c "import json,sys; v=json.load(sys.stdin)['$1']; print(v if isinstance(v,str) else json.dumps(v))"
}
wait_until() { # ns timestamp
  local t=$(( $1 / 1000000000 + 5 ))
  while [ "$(date +%s)" -lt "$t" ]; do sleep 10; done
}

log "market $M on $F as $A"
wait_until "$(mfield auction_commit_end)"
while read -r label side qty limit salt escrow; do
  [ -z "$label" ] && continue
  tx reveal_bid "{\"market_id\":\"$M\",\"side\":\"${side#side=}\",\"qty\":\"${qty#qty=}\",\"limit_cents\":${limit#limit=},\"salt\":\"${salt#salt=}\"}" "30 Tgas"
done < "$BIDS"

wait_until "$(mfield auction_reveal_end)"
tx clear_auction "{\"market_id\":\"$M\"}" "300 Tgas"
log "status=$(mfield status) opening_price_cents=$(mfield opening_price_cents) pairs=$(mfield pairs)"

PS=$(mfield period_start); PE=$(mfield period_end)
STEP=$(( (PE - PS) / OBS ))
for i in $(seq 0 $((OBS - 1))); do
  wait_until $(( PS + i * STEP ))
  KEEPER_ACCOUNT="$A" KEEPER_MARKETS="$M" FLATRATE_ID="$F" node scripts/keeper.mjs --once
done
log "observations=$(mfield observations | $PY -c 'import json,sys; print(len(json.load(sys.stdin)))')"

wait_until "$PE"
tx settle "{\"market_id\":\"$M\"}"
log "status=$(mfield status)"
tx redeem "{\"market_id\":\"$M\"}"
tx withdraw_refund "{}" "50 Tgas"
log "collateral=$(mfield collateral) done"
