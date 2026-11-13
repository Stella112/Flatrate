# Flatrate

Flatrate lets AI teams and agents lock next week's GPU price on NEAR testnet. A buyer of compute takes a **Long** claim, which pays more when the H100 price per GPU-hour rises. A seller of compute takes the **Short** claim, which pays more when it falls. One fUSD of collateral backs one Long plus one Short. Settlement uses the median of five public GPU price APIs, fetched inside OutLayer and posted on-chain one price per time slot, so no operator sets the price. Testnet only; fUSD is a test token.

Other submission by the same builder: **Facedown**, https://github.com/Stella112/facedown

## Try it

1. Open **https://flatrate-gpu.vercel.app/**. Click "Create guest account": the page makes a key in your browser and a sponsor sends 0.5 testnet NEAR and 100 fUSD to its account. No wallet needed.
2. Open a DEMO market. If the latest one is settled, click "Roll the next DEMO market" (anyone can): a new market opens with its band centred on the last settled average. Commit a sealed bid within 3 minutes, reveal within the next 2, then clear. The keeper fills one price per 1-minute slot for 8 minutes; settle and redeem after that.
3. Open **https://flatrate-gpu.vercel.app/#/proof**: every recorded price with its OutLayer request id, transaction, per-source prices and attestation checks; four buttons that try to break the contract.
4. Open **https://flatrate-gpu.vercel.app/?demo=scripted** for a step-by-step replay of a finished testnet market, with a "Scripted demo" badge and no transactions.
5. From a clone: `make vendor && make verify` (Linux or macOS, see "Run the checks"). Expect a table and `RESULT: no FAIL` in a few minutes.
6. From a clone: `make test`. Expect the sandbox suite to end with `test result: ok. 29 passed` (several minutes).
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
| Live WEEKLY market | market 10: auction until 2026-11-14 13:23 UTC, then 28 six-hour slots until 2026-11-21 |

## What NEAR does here (remove NEAR and it stops)

- **OutLayer price fetching inside an enclave.** The settlement price is the output of a pinned wasm run by OutLayer at the contract's request. The contract accepts it only through its own callback for a request id it issued, one per time slot.
- **Yield and resume.** `request_execution` returns a promise that resumes when OutLayer's worker answers; the callback gets the oracle's JSON.
- **Public `roll_market`.** Opening, observing, clearing, settling and rolling are all public calls; the owner can only open the first market of each template. (Deleting the contract's own full-access keys is pending, see Limitations.)
- **Function-call access keys.** The keeper's account holds only a key that may call `clear_auction`, `request_observation`, `settle` and `roll_market` on the flatrate contract.
- **Implicit accounts in the browser.** Guests get an implicit account from a key made in the browser; every action on the site is a transaction signed by that key.
- **NEAR Intents quotes.** The "Pay from any chain" page asks NEAR Intents 1Click (and the Aurora Intents API, when its key is configured) for a dry-run quote: what paying in USDC from an EVM chain would cost.

## Tracks

- **Unstoppable DeFi**: a fully collateralised two-sided market with no admin price setting, no admin settlement override and public market rolling.
- **NFTs That Do Something**: a Cover Note is a transferable claim on a future payout; whoever holds it at settlement is paid.
- **Privacy & Private AI**: bids stay sealed until the reveal window, and prices come from a pinned program run by OutLayer in a TEE, with an attestation per price.

## Run the checks

Requirements: Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build; on Windows use WSL), Rust (rustup) with targets `wasm32-unknown-unknown` and `wasm32-wasip2`, `cargo-near` 0.23, Node 20+. Optional: Docker (reproducible oracle build) and OutLayer's test runner (`make oracle-runner`, several minutes once).

```bash
make vendor          # clone OutLayer at the pinned commit (build script + test runner)
make verify          # shared tests, builds, oracle, read-only testnet checks; prints a table (a few minutes)
make test            # shared, contract unit, oracle and sandbox tests (several minutes)
make demo-scripted   # replay a recorded testnet market in the sandbox
make oracle-repro    # build oracle/ twice in OutLayer's pinned compiler image; prints the sha256
```

`make verify` reports PASS, FAIL, SKIP (a tool is missing: Docker or the OutLayer runner) or PENDING (the contract's full-access key, removed only after the owner approves). Every claim in this README has a row in [CLAIMS.md](CLAIMS.md) with a test or a testnet transaction.

## Limitations

- **Testnet only, test stablecoin.** fUSD has a public faucet. No mainnet contracts.
- **Both sides seeded by us.** The bids in the recorded markets come from our own accounts (the guest in market 11 bid on both sides); there are no outside traders.
- **Revealed bids are public.** Bids are hidden while bidding, but the reveal makes them public before clearing. Clearing inside an enclave is not built.
- **The feed is only as good as its sources.** Five public APIs; Shadeform aggregates other clouds, including Verda, so they are not fully independent. A source can change its API; a run needs at least 3 working sources.
- **The contract does not verify a TEE attestation itself.** It trusts the OutLayer contract and the pinned code version. The Proof page and `make verify` fetch OutLayer's attestation record for each price and compare its wasm, input and output hashes, but do not verify the TDX quote. OutLayer's governance still runs on keys its operators hold (near-outlayer `docs/NON_CUSTODIAL.md`), and its testnet had an outage during our first DEMO market (BLOCKERS.md B10); a market without enough slots settles void.
- **Keys and upgrades.** The contract still has its full-access key until the owner approves deleting it (`go keyless`). After that it cannot be upgraded. The previous version was replaced in place on 2026-11-13 with a `reset` init (the old markets stay on the explorer only).
- **Paying from another chain is a quote only.** 1Click and Aurora have no testnet; quotes are mainnet prices and nothing is sent. 1Click's quote endpoint timed out at times during the build.
- **Sponsored guest accounts.** The sponsor pays 0.5 testnet NEAR per guest; its rate limit (3 per IP per hour) is kept in serverless memory, so it is best-effort.
- **Rhea.** `ref-finance-101.testnet` is live on testnet (2961 pools), but no Flatrate pool exists: the Rhea extra is not built.
- **Not built.** Agent auto-hedging, encrypted auction, token price market, chain-signature pay-in, mainnet payments, wallet login.

## Repository

`contracts/flatrate` (markets, auction, claims, slots, settlement, rolling, Cover Notes), `contracts/fusd` (test stablecoin), `contracts/mock-outlayer` (sandbox only, never deployed), `shared/` (payoff, median, auction clearing), `oracle/` (OutLayer component), `web/` (site and `web/api` serverless functions), `scripts/` (keeper, verify, demo helpers), `fixtures/` (recorded API responses and a recorded market). Build log with real output: [PROGRESS.md](PROGRESS.md). Sources for every external fact: [FACTS.md](FACTS.md). Open items: [BLOCKERS.md](BLOCKERS.md). License: MIT ([LICENSE](LICENSE)).

Built at the NEAR Hackathon.
