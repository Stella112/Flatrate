# Flatrate

Flatrate lets AI teams and agents lock next week's GPU price on NEAR testnet. A buyer of compute takes a **Long** claim, which pays more when the H100 price per GPU-hour rises. A seller of compute takes the **Short** claim, which pays more when it falls. One fUSD of collateral backs one Long plus one Short. Settlement uses the median of five public GPU price APIs, fetched inside OutLayer and posted on-chain one price per time slot, so no operator sets the price. Testnet only; fUSD is a test token.

A second deployment of the same contract (extra 2) runs markets on an LLM token price instead: Llama 3.3 70B output tokens, USD per 1M, the median across seven OpenRouter providers. The site switches between the two with the "H100 GPU-hour" / "LLM tokens" buttons.

Other submission by the same builder: **Facedown**, https://github.com/Stella112/facedown

## Try it

1. Open **https://flatrate-gpu.vercel.app/** for the landing page (the story, a payoff calculator and live testnet numbers), then **https://flatrate-gpu.vercel.app/#/app** for the app. In the app's sidebar click "Create guest account": the page makes a key in your browser and a sponsor sends 0.1 testnet NEAR and 100 fUSD to its account. No wallet needed. The sidebar switches between the three contracts (H100 GPU-hour, LLM tokens, H100 private auction).
2. Open a DEMO market. If the latest one is settled, click "Roll the next DEMO market" (anyone can): a new market opens with its band centred on the last settled average. Commit a sealed bid within 3 minutes, reveal within the next 2, then clear. The keeper fills one price per 1-minute slot for 8 minutes; settle and redeem after that.
3. Open **https://flatrate-gpu.vercel.app/#/proof**: every recorded price with its OutLayer request id, transaction, per-source prices and attestation checks; four buttons that try to break the contract.
4. Open **https://flatrate-gpu.vercel.app/?demo=scripted#/app** for a step-by-step replay of a finished testnet market, with a "Scripted demo" badge and no transactions.
5. From a clone: `make vendor && make verify` (Linux or macOS, see "Run the checks"). Expect a table and `RESULT: no FAIL` in a few minutes.
6. From a clone: `make test`. Expect the sandbox suite to end with `test result: ok. 30 passed` (several minutes).
7. From a clone: `make demo-scripted`. Expect `SCRIPTED DEMO PASS: sandbox replay equals testnet market 2`.

## How it works

1. **Market.** Markets come from two templates fixed when the contract was initialised: `DEMO` (3 min commits, 2 min reveals, 8 one-minute slots, 3 needed) and `WEEKLY` (24 h, 12 h, 28 six-hour slots, 14 needed). The owner opened the first market of each; every later one is opened by `roll_market`, which anyone can call once the previous market of that template is settled or void. A rolled market's band is the previous settled average x0.6 (floor) and x1.4 (cap), rounded down to 10 cents.
2. **Sealed opening auction.** Bidders commit `sha256(borsh(market_id, side, qty, limit_cents, salt))` with an fUSD escrow, then reveal. `clear_auction` picks one price P (most matched, then closest to 50, then lower): Long pays P cents, Short pays 100 − P.
3. **Pairs.** Each matched pair mints one Long and one Short claim backed by 1 fUSD. Unused escrow, including unrevealed bids, is refundable. Afterwards anyone can mint or redeem pairs at 1 fUSD.
4. **Observations.** During the period anyone calls `request_observation` once per time slot. The contract pays OutLayer from its own balance and calls `request_execution` on `outlayer.testnet` with the pinned oracle version; the oracle fetches five APIs and returns each price and the median. The result comes back through yield and resume to `on_observation` and is written into the slot it was requested for. A failed or rejected result frees the slot.
5. **Settlement.** After the period anyone calls `settle`. The average is the mean of the filled slots. Long pays `clamp((avg − floor) / (cap − floor), 0, 1)` fUSD per claim, Short pays the rest. With too few filled slots the market waits, then settles void at 0.50 / 0.50.
6. **Cover Notes.** Claims can be wrapped in a Cover Note NFT (NEP-171). The note can be transferred; whoever holds it after settlement redeems it.

```
 bidders ──commit/reveal──▶ flatrate contract ──request_execution (one per slot)──▶ outlayer.testnet
   │                          │  ▲                                                │ pinned oracle wasm
   │      fUSD (NEP-141)      │  └──────── on_observation (yield/resume) ─────────┘ akash, vast, runpod,
   ▼                          ▼                                                      shadeform, verda → median
 Long / Short claims ──wrap──▶ Cover Note (NEP-171) ──after settle──▶ redeem      roll_market ──▶ next market
```

## Deployed on testnet

| What | Account / value |
|---|---|
| Flatrate contract | `flatrate.flatrate-maris.testnet` |
| Test stablecoin fUSD (NEP-141, 6 decimals, public faucet) | `fusd.flatrate-maris.testnet` |
| Owner (opened the first market of each template) | `flatrate-maris.testnet` |
| OutLayer contract and oracle project | `outlayer.testnet`, `flatrate-maris.testnet/flatrate-oracle` |
| Pinned oracle version (fixed at init) | `6c252d52804cb3f356ab1f5718069a57ab2d2fb96ad14e21eb18277fbcdaaf44`, a reproducible build of `oracle/` |
| Price sources | Akash (`console-api.akash.network/v1/gpu-prices`), Vast.ai (bundles search), RunPod (GraphQL `gpuTypes`), Shadeform (`/v1/instances/types`), Verda (`/v1/instance-types`); selection rules in [FACTS.md](FACTS.md) section 4 |
| Keeper (function-call key only) | `54cf686374f338d49d9a8e12e9912b6fa7f10ee7d5fcc561b6faa4abfac6158e` |
| Guest sponsor | `flatrate-sponsor.testnet` (key only in the hosting provider) |
| Fully settled DEMO market | market 11: every transaction in [PROGRESS.md](PROGRESS.md) ("Build prompt v2") and [CLAIMS.md](CLAIMS.md) |
| Live WEEKLY market | market 10: commits until 2026-11-14 11:32 UTC, reveals until 23:32 UTC, then 28 six-hour slots until 2026-11-21 23:32 UTC |
| Token-price contract (extra 2) | `tokens.flatrate-maris.testnet`: the same contract with `index: TOKEN_PRICE`, oracle project `flatrate-maris.testnet/flatrate-oracle-tokens`, pinned version `b7d4679204d59903d538196ff90e7f66f59962b680ee4961b23a43ace46548b3` (reproducible build of `oracle-tokens/`); full-access key still present (not keyless) |
| Token-price keeper | `keeper.flatrate-maris.testnet` (function-call key only: `clear_auction`, `request_observation`, `settle`, `roll_market` on the token contract) |
| Contract builds | `flatrate.flatrate-maris.testnet` runs the build of commit `e2420a4` (wasm sha256 `1965f276…`; keyless, so it cannot pick up later changes). `tokens.flatrate-maris.testnet` runs the build of this branch's contract with the `index` init field (wasm sha256 `82c138c3…`). The only contract change after `e2420a4` is that field (default `H100_HOURLY`) and the Cover Note title using it |
| Private-auction contract (extra) | `sealed.flatrate-maris.testnet`: H100 markets whose opening auction is sealed to an OutLayer enclave (project `flatrate-maris.testnet/flatrate-auction`, pinned version `940f44d7…`) and cleared inside it; enclave key `78f5091a…`; full-access key still present |
| Private-auction keeper | `sealkeeper.flatrate-maris.testnet` (function-call key only: `clear_auction`, `request_observation`, `settle`, `roll_market` on the sealed contract) |
| Claim token and Rhea pool (extra) | `frl11.flatrate-maris.testnet` (NEP-141 "FRL11": Long claims of sealed market 11, 1 token = 1 claim); Rhea (`ref-finance-101.testnet`) pool 2978, FRL11 / fUSD, fee 0.25% |
| Chain signatures (extra, reduced) | `holder.flatrate-maris.testnet` controls Base Sepolia `0xF8677CE5ebc12A0E396e2747e4917C7C6E2dBe61` through `v1.signer-prod.testnet` (path `flatrate-payout`); `scripts/chainsig/sepolia.mjs` |
| Hedging agent (extra) | `agent.flatrate-maris.testnet`: a function-call key limited to the flatrate contract, plus a full-access key used only to commit bids; ran DEMO market 13 end to end |

## What NEAR does here (remove NEAR and it stops)

- **OutLayer price fetching inside an enclave.** The settlement price is the output of a pinned wasm run by OutLayer at the contract's request. The contract accepts it only through its own callback for a request id it issued, one per time slot.
- **Yield and resume.** `request_execution` returns a promise that resumes when OutLayer's worker answers; the callback gets the oracle's JSON.
- **Keyless contract with public `roll_market`.** The contract account has no access keys, so nobody can upgrade it or act as it. Observing, clearing, settling and rolling are public calls; market 12 was rolled by a guest account after the keys were deleted.
- **Function-call access keys.** The keeper's account holds only a key that may call `clear_auction`, `request_observation`, `settle` and `roll_market` on the flatrate contract. The hedging agent rolls, reveals, withdraws refunds and redeems with a key limited to the flatrate contract (allowance 0.25 NEAR).
- **Implicit accounts in the browser.** Guests get an implicit account from a key made in the browser; every action on the site is a transaction signed by that key. Visitors can instead sign in with a NEAR wallet (NEAR Wallet Selector: Meteor, MyNearWallet, Intear).
- **Chain signatures.** A NEAR account signs transactions for an address on Base Sepolia through NEAR's MPC signer: `holder.flatrate-maris.testnet` sent 0.001 test ETH from its derived address (sign request [6vjfR3iW](https://testnet.nearblocks.io/txns/6vjfR3iW1cFiqyfk4xaUPBmJXJfax5nthG67Z2oqttSA), Base Sepolia [0xa9924054](https://sepolia.basescan.org/tx/0xa9924054d5f471a131709ac2e8d5f288f73d8446a9622256ac2ed0d496209898)).
- **NEAR Intents quotes.** The "Pay from any chain" page asks NEAR Intents 1Click and the Aurora Intents API for a dry-run quote, shown side by side: what paying in USDC from an EVM chain would cost.

## Tracks

- **Unstoppable DeFi**: a fully collateralised two-sided market with no admin price setting, no admin settlement override and public market rolling.
- **NFTs That Do Something**: a Cover Note is a transferable claim on a future payout; whoever holds it at settlement is paid.
- **Agent Economy**: `scripts/agent/hedge-agent.mjs` reads a usage forecast, sizes a Long hedge (GPU-hours x band x hedge ratio), prices it from the last settled average, and runs the auction to redemption on its own account. It hedged DEMO market 13 on testnet.
- **Autonomous Business**: the contract has no access keys; markets are rolled, cleared, priced and settled by public calls. A keeper holding only a function-call key runs them, and anyone can run the same keeper. DEMO market 14 was rolled by a fresh guest and cleared, priced and settled by the keeper with no operator action.
- **Privacy & Private AI**: bids stay sealed until the reveal window, and prices come from a pinned program run by OutLayer in a TEE, with an attestation per price.

## Run the checks

Requirements: Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build; on Windows use WSL), Rust (rustup) with targets `wasm32-unknown-unknown` and `wasm32-wasip2`, `cargo-near` 0.23, Node 20+. Optional: Docker (reproducible oracle build) and OutLayer's test runner (`make oracle-runner`, several minutes once).

```bash
make vendor          # clone OutLayer at the pinned commit (build script + test runner)
make verify          # shared tests, builds, oracle, read-only testnet checks; prints a table (a few minutes)
make test            # shared, contract unit, oracle and sandbox tests (several minutes)
make demo-scripted   # replay a recorded testnet market in the sandbox
make test-agent      # hedging agent bid planning tests (node, no network)
make agent-dry       # what the agent would bid on the latest markets (read-only, no keys)
ORACLE_DIR=oracle-tokens bash scripts/oracle-reproducible.sh --twice   # same for the token-price oracle
make oracle-repro    # build oracle/ twice in OutLayer's pinned compiler image; prints the sha256
```

`make verify` reports PASS, FAIL, SKIP (a tool is missing: Docker or the OutLayer runner) or INFO. Every claim in this README has a row in [CLAIMS.md](CLAIMS.md) with a test or a testnet transaction.

## Limitations

- **Testnet only, test stablecoin.** fUSD has a public faucet. No mainnet contracts.
- **Both sides seeded by us.** The bids in the recorded markets come from our own accounts (the guest in market 11 bid on both sides); there are no outside traders.
- **Revealed bids are public.** Bids are hidden while bidding, but the reveal makes them public before clearing. Clearing inside an enclave is not built.
- **The feed is only as good as its sources.** Five public APIs; Shadeform aggregates other clouds, including Verda, so they are not fully independent. A source can change its API; a run needs at least 3 working sources.
- **The contract does not verify a TEE attestation itself.** It trusts the OutLayer contract and the pinned code version. The Proof page and `make verify` fetch OutLayer's attestation record for each price and compare its wasm, input and output hashes, but do not verify the TDX quote. OutLayer's governance still runs on keys its operators hold (near-outlayer `docs/NON_CUSTODIAL.md`). Intel TDX itself has had published attacks, for example TDXdown (Wilke, Sieck, Eisenbarth, ACM CCS 2024, https://research.uni-luebeck.de/en/publications/tdxdown-single-stepping-and-instruction-counting-attacks-against-/). OutLayer's testnet also had an outage during our first DEMO market (BLOCKERS.md B10); a market without enough slots settles void.
- **No upgrades.** The contract's keys are deleted, so a bug cannot be fixed; if a price source changes its format, markets settle at the midpoint until a new contract is deployed. The previous version was replaced in place on 2026-11-13 with a `reset` init, before the keys were deleted (the old markets stay on the explorer only).
- **Paying from another chain is a quote only.** 1Click and Aurora have no testnet; quotes are mainnet prices and nothing is sent. 1Click's quote endpoint timed out at times during the build.
- **Sponsored guest accounts.** The sponsor pays 0.1 testnet NEAR per guest (a full market lifecycle cost a guest 0.0059 NEAR); its rate limit (3 per IP per hour) is kept in serverless memory, so it is best-effort.
- **Rhea pool is seeded by us.** Pool 2978 holds liquidity from our own account (10 FRL11 + 5 fUSD); the one swap in it is ours (holder paid 1 fUSD for 1.663192 FRL11, [9ZnA1WBW](https://testnet.nearblocks.io/txns/9ZnA1WBWo5mpS85QeSpZ7Dm4Tp3P1YsVZvwQrjYEWU8C)). Claim tokens exist only for the market a token contract was deployed for.
- **Private auction not yet cleared on testnet.** The enclave key was issued through the contract ([GjkYcbU6](https://testnet.nearblocks.io/txns/GjkYcbU6dXeYfCyd2oWKuDPGFDGv1cjT7CqdV6yc8Q7V)) and sealed bids were committed (sealed market 10), but OutLayer testnet failed every execution on 2026-11-14 from about 09:15 UTC (BLOCKERS.md B14), so the enclave clearing ran only in the sandbox tests; market 10 was abandoned and both bidders refunded in full. The contract also trusts the enclave's fills as OutLayer returns them (it checks escrow and balance, not the bids' contents).
- **The agent's commit key is full-access.** A bid is committed through fUSD `ft_transfer_call`, which needs 1 yoctoNEAR attached, and NEAR function-call keys cannot attach a deposit (BLOCKERS.md B11). The agent therefore commits with a full-access key on its own account; its limit is the fUSD it holds and `max_spend_fusd` per bid. Every other agent call uses the flatrate-only key. The contract is keyless, so a deposit-free commit path cannot be added.
- **Token-price index has one upstream.** All seven prices come from one OpenRouter endpoint, so OpenRouter is a single point of trust for that index; the H100 index uses five separate APIs. The token contract still has its full-access key.
- **Wallet login not yet used with a real wallet.** The site loads Meteor, MyNearWallet and Intear and starts their sign-in; no transaction has been signed through a wallet yet (the guest flow is the tested path).
- **Cross-chain is signing only.** The chain-signature script moves test ETH on Base Sepolia from an address a NEAR account controls; no fUSD or claim crosses chains, because that needs a bridge with an operator (BLOCKERS.md B13).
- **Not built.** Real mainnet payments (BLOCKERS.md B12).

## Repository

`contracts/flatrate` (markets, auction, claims, slots, settlement, rolling, Cover Notes), `contracts/fusd` (test stablecoin), `contracts/mock-outlayer` (sandbox only, never deployed), `shared/` (payoff, median, auction clearing), `oracle/` (OutLayer component, H100), `oracle-tokens/` (OutLayer component, LLM tokens), `web/` (site and `web/api` serverless functions), `scripts/` (keeper, hedging agent in `scripts/agent/`, chain signatures in `scripts/chainsig/`, verify, demo helpers), `fixtures/` (recorded API responses and a recorded market). Build log with real output: [PROGRESS.md](PROGRESS.md). Sources for every external fact: [FACTS.md](FACTS.md). Open items: [BLOCKERS.md](BLOCKERS.md). License: MIT ([LICENSE](LICENSE)).

Built at the NEAR Hackathon.
