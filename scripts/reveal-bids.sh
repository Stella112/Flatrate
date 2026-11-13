#!/usr/bin/env bash
# Reveals committed sealed bids once the market's commit phase ends. Each bid is signed by its own account.
#
# Usage: MARKET=10 BIDS=target/market10-bids.txt bash scripts/reveal-bids.sh
# BIDS lines (gitignored, written at commit time):
#   "<label> side=<Long|Short> qty=<n> limit=<cents> salt=<salt> escrow=<micro> account=<id> hash=<hex>"
# Needs: near (near-cli-rs) with each account's key in ~/.near-credentials, python3.
set -uo pipefail
PY=python3; python3 -c "" >/dev/null 2>&1 || PY=python
cd "$(dirname "$0")/.."
F="${FLATRATE_ID:-flatrate.flatrate-maris.testnet}"
M="${MARKET:?set MARKET}"
BIDS="${BIDS:?set BIDS}"

log() { echo "$(date -u +%FT%TZ) $*"; }
strip() { sed 's/\x1b\[[0-9;]*m//g'; }
mfield() {
  near contract call-function as-read-only "$F" get_market json-args "{\"market_id\":\"$M\"}" network-config testnet now </dev/null 2>&1 \
    | strip | sed -n '/^{/,/^}/p' | $PY -c "import json,sys; v=json.load(sys.stdin)['$1']; print(v if isinstance(v,str) else json.dumps(v))"
}
field() { tr ' ' '\n' <<<"$1" | grep "^$2=" | cut -d= -f2-; }

CE=$(mfield auction_commit_end); RE=$(mfield auction_reveal_end)
t=$(( CE / 1000000000 + 5 ))
log "market $M: waiting for the reveal window ($(date -u -d @$t +%FT%TZ 2>/dev/null || echo $t))"
while [ "$(date +%s)" -lt "$t" ]; do sleep 30; done
if [ "$(date +%s)" -ge $(( RE / 1000000000 )) ]; then log "reveal window already closed"; exit 1; fi

while read -r line; do
  [ -z "$line" ] && continue
  label=${line%% *}
  args="{\"market_id\":\"$M\",\"side\":\"$(field "$line" side)\",\"qty\":\"$(field "$line" qty)\",\"limit_cents\":$(field "$line" limit),\"salt\":\"$(field "$line" salt)\"}"
  for i in 1 2 3; do
    out=$(near contract call-function as-transaction "$F" reveal_bid json-args "$args" prepaid-gas '30 Tgas' \
      attached-deposit '0 NEAR' sign-as "$(field "$line" account)" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip)
    grep -q 'Transaction ID' <<<"$out" && break
    log "$label: no transaction sent, retry $i"; sleep 15
  done
  log "$label reveal tx=$(grep -oE 'Transaction ID: \w+' <<<"$out" | head -1 | cut -d' ' -f3)"
  grep -iE 'panicked|ERR_' <<<"$out" | head -3 | sed 's/^/    ERR /'
done < "$BIDS"
log "done; the keeper (or anyone) calls clear_auction after $(date -u -d @$(( RE / 1000000000 )) +%FT%TZ 2>/dev/null || echo $RE)"
