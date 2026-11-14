#!/usr/bin/env bash
# Extra (Rhea pool): Flatrate Long claims of one market as a NEP-141 token, traded against fUSD in
# a Rhea (Ref) simple pool on testnet. Call shapes from ref-finance/docs (FACTS.md section 17).
#
# Steps (each a real testnet transaction):
#   lp     LP wraps claims: Flatrate claim_transfer_call -> the claim token mints FRL<market> to the LP
#   lp     registers on the exchange, deposits FRL and fUSD, add_liquidity to the pool
#   trader registers, deposits fUSD, swaps fUSD -> FRL, withdraws FRL, unwraps it into Flatrate claims
#
# Usage: LP=<account> TRADER=<account> POOL=<id> TOKEN=<claim token> MARKET=<id> \
#        LP_CLAIMS=<units> LP_FUSD=<units> SWAP_FUSD=<units> bash scripts/rhea-pool.sh
# Needs: near (near-cli-rs) with both accounts' keys in ~/.near-credentials (legacy keychain).
set -uo pipefail
REF="${REF:-ref-finance-101.testnet}"
FLATRATE_ID="${FLATRATE_ID:-sealed.flatrate-maris.testnet}"
FUSD="${FUSD:-fusd.flatrate-maris.testnet}"
: "${LP:?}" "${TRADER:?}" "${POOL:?}" "${TOKEN:?}" "${MARKET:?}" "${LP_CLAIMS:?}" "${LP_FUSD:?}" "${SWAP_FUSD:?}"

strip() { sed 's/\x1b\[[0-9;]*m//g'; }
c() { # c <signer> <contract> <method> <json> <deposit> [gas]
  local out
  out=$(near contract call-function as-transaction "$2" "$3" json-args "$4" prepaid-gas "${6:-100} Tgas" attached-deposit "$5" \
    sign-as "$1" network-config testnet sign-with-legacy-keychain send </dev/null 2>&1 | strip)
  echo "$3 by $1: $(grep -oE 'Transaction ID: \w+' <<<"$out" | head -1) $(grep -oE 'ERR_\w+|E[0-9]+: [^"\\]*|panicked at [^"\\]*' <<<"$out" | head -1)"
}

echo "== LP wraps $LP_CLAIMS Long claim units of market $MARKET into $TOKEN"
c "$LP" "$FLATRATE_ID" claim_transfer_call "{\"receiver_id\":\"$TOKEN\",\"market_id\":\"$MARKET\",\"side\":\"Long\",\"amount\":\"$LP_CLAIMS\",\"msg\":\"\"}" '0 NEAR'
echo "== LP deposits and adds liquidity to pool $POOL"
c "$LP" "$TOKEN" ft_transfer_call "{\"receiver_id\":\"$REF\",\"amount\":\"$LP_CLAIMS\",\"msg\":\"\"}" '1 yoctoNEAR'
c "$LP" "$FUSD" ft_transfer_call "{\"receiver_id\":\"$REF\",\"amount\":\"$LP_FUSD\",\"msg\":\"\"}" '1 yoctoNEAR'
c "$LP" "$REF" add_liquidity "{\"pool_id\":$POOL,\"amounts\":[\"$LP_CLAIMS\",\"$LP_FUSD\"]}" '0.01 NEAR'

echo "== trader swaps $SWAP_FUSD fUSD units for $TOKEN"
c "$TRADER" "$REF" storage_deposit '{}' '0.1 NEAR' 30
c "$TRADER" "$REF" register_tokens "{\"token_ids\":[\"$TOKEN\",\"$FUSD\"]}" '1 yoctoNEAR' 30
c "$TRADER" "$TOKEN" storage_deposit "{\"account_id\":\"$TRADER\"}" '0.00125 NEAR' 30
c "$TRADER" "$FUSD" ft_transfer_call "{\"receiver_id\":\"$REF\",\"amount\":\"$SWAP_FUSD\",\"msg\":\"\"}" '1 yoctoNEAR'
c "$TRADER" "$REF" swap "{\"actions\":[{\"pool_id\":$POOL,\"token_in\":\"$FUSD\",\"amount_in\":\"$SWAP_FUSD\",\"token_out\":\"$TOKEN\",\"min_amount_out\":\"1\"}]}" '1 yoctoNEAR'
GOT=$(near contract call-function as-read-only "$REF" get_deposits json-args "{\"account_id\":\"$TRADER\"}" network-config testnet now </dev/null 2>&1 | strip | grep -oE "\"$TOKEN\": \"[0-9]+\"" | grep -oE '[0-9]+"$' | tr -d '"')
echo "trader's $TOKEN deposit on the exchange: ${GOT:-?}"
[ -n "${GOT:-}" ] && [ "$GOT" != 0 ] || { echo "nothing to withdraw"; exit 1; }
c "$TRADER" "$REF" withdraw "{\"token_id\":\"$TOKEN\",\"amount\":\"$GOT\"}" '1 yoctoNEAR'
echo "== trader unwraps $GOT $TOKEN into Flatrate claims"
c "$TRADER" "$TOKEN" unwrap "{\"amount\":\"$GOT\"}" '1 yoctoNEAR'
near contract call-function as-read-only "$FLATRATE_ID" claim_balance_of json-args "{\"account_id\":\"$TRADER\",\"market_id\":\"$MARKET\",\"side\":\"Long\"}" network-config testnet now </dev/null 2>&1 | strip | grep -E '^"' | sed "s/^/trader Long claims of market $MARKET: /"
