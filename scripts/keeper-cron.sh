#!/usr/bin/env bash
# One keeper pass from cron. Install (example, every 6 hours):
#   15 */6 * * * KEEPER_ACCOUNT=<account> KEEPER_MARKETS=5 $HOME/flatrate/scripts/keeper-cron.sh
# The account's key must be in ~/.near-credentials/testnet/<account>.json (near-cli legacy keychain).
set -u
export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
cd "$(dirname "$0")/.."
: "${KEEPER_ACCOUNT:?set KEEPER_ACCOUNT}"
node scripts/keeper.mjs --once >> "${KEEPER_LOG:-$HOME/flatrate-keeper.log}" 2>&1
