# Flatrate

Flatrate lets AI teams and agents lock next week's GPU price on NEAR testnet. A buyer of compute takes a **Long** claim, which pays more when the H100 price per GPU-hour rises. A seller of compute takes the **Short** claim, which pays more when the price falls. One fUSD of collateral backs one Long plus one Short. Settlement uses the median of five public GPU price APIs, fetched inside OutLayer and posted on-chain, so no operator sets the price. Testnet only; fUSD is a test token.

Other submission by the same builder: **Facedown**, https://github.com/Stella112/facedown

## Try it

1. Open **https://flatrate-gpu.vercel.app/**. Click "Create guest account": the page makes a key in your browser and a sponsor sends 0.5 testnet NEAR and 100 fUSD to its account. No wallet needed.
2. Open **https://flatrate-gpu.vercel.app/#/proof**: every on-chain price observation with its OutLayer request id, transaction and per-source prices; four buttons that try to break the contract.
3. Open **https://flatrate-gpu.vercel.app/?demo=scripted** for a replay of a finished testnet market, step by step, with a "Scripted demo" badge and no transactions.
4. From a clone: `make verify` (needs Linux or macOS, see "Run the checks"). Expect a table of checks and `RESULT: no FAIL`.
5. From a clone: `make demo-scripted`. Expect `SCRIPTED DEMO PASS: sandbox replay equals testnet market 2`.

## How it works

1. **Market.** The owner creates a market for one index (H100 price per GPU-hour), one period, a floor and a cap. Each market pins the oracle version (the wasm's sha256) it will use.
2. **Sealed opening auction.** Bidders commit `sha256(borsh(market_id, side, qty, limit_cents, salt))` with an fUSD escrow. After the commit window they reveal. `clear_auction` picks one price P (the most matched, then closest to 50, then lower): Long pays P cents, Short pays 100 − P.
3. **Pairs.** Each matched pair mints one Long and one Short claim backed by 1 fUSD. Unused escrow, including unrevealed bids, is refundable. After the auction anyone can mint or redeem pairs at 1 fUSD.
4. **Observations.** During the period anyone calls `request_observation` and pays OutLayer's fee. The contract calls `request_execution` on `outlayer.testnet`; the oracle fetches five APIs and returns each price and the median. The result comes back through yield and resume to `on_observation`.
5. **Settlement.** After the period anyone calls `settle`. Long pays `clamp((avg − floor) / (cap − floor), 0, 1)` fUSD per claim, Short pays the rest. With too few observations the market waits, then settles void at 0.50 / 0.50.
6. **Cover Notes.** Claims can be wrapped in a Cover Note NFT (NEP-171). The note can be transferred; whoever holds it after settlement redeems it.

```
 bidders ──commit/reveal──▶ flatrate contract ──request_execution──▶ outlayer.testnet
   │                          │  ▲                                   │ (oracle wasm, pinned sha256)
   │      fUSD (NEP-141)      │  └──── on_observation (yield/resume) ─┘  akash, vast, runpod,
   ▼                          ▼                                          shadeform, verda → median
 Long / Short claims ──wrap──▶ Cover Note (NEP-171) ──after settle──▶ redeem for fUSD
```

## Deployed on testnet

| What | Account / value |
|---|---|
| Flatrate contract | `flatrate.flatrate-maris.testnet` |
| Test stablecoin fUSD (NEP-141, 6 decimals, public faucet) | `fusd.flatrate-maris.testnet` |
| Owner (creates markets) | `flatrate-maris.testnet` |
| OutLayer contract | `outlayer.testnet` |
| Oracle project | `flatrate-maris.testnet/flatrate-oracle` |
| Oracle version pinned for new markets | `6c252d52804cb3f356ab1f5718069a57ab2d2fb96ad14e21eb18277fbcdaaf44` (reproducible build of `oracle/`, see below) |
| Oracle version pinned by markets 0–4 | `f55f27004faa014bb08665f5cf55fab21418c7c2004d1e17fc8758580a8694fa` (same source, earlier non-reproducible build) |
| Guest sponsor | `flatrate-sponsor.testnet` (key only in the hosting provider) |
| Keeper (requests observations every 6 h for market 5) | `54cf686374f338d49d9a8e12e9912b6fa7f10ee7d5fcc561b6faa4abfac6158e` |

Markets: 2, 3 and 4 settled with real OutLayer observations; 0 settled void; 1 was the first weekly market (no bids); **5 is the weekly market**, period 2026-11-12 21:49 → 2026-11-19 22:00 UTC, 7 observations required.

## What NEAR does here (remove NEAR and it stops)

- **OutLayer attested price fetching.** The price for settlement is the output of a pinned wasm run by OutLayer, requested by the contract itself. The contract only accepts it through its own callback for a request id it issued.
- **Yield and resume.** `request_execution` returns a promise that resumes when OutLayer's worker answers; the contract's callback gets the oracle's JSON as the promise result.
- **Implicit accounts and function-call transactions in the browser.** Guests get an implicit account from a key made in the browser; every action on the site is a transaction signed by that key.
- **NEP-141 and NEP-171.** fUSD is a NEP-141 token with `ft_transfer_call` into the contract (mint pairs, commit bids). Cover Notes are NEP-171 NFTs with NEP-177 metadata and NEP-181 enumeration.
- **NEAR Intents (1Click) quotes.** The "Pay from any chain" page asks 1Click for a dry-run quote: what paying in USDC from an EVM chain would cost. 1Click has no testnet, so nothing is sent.

## Tracks

- **Unstoppable DeFi**: a fully collateralised two-sided market with no admin price setting and no admin settlement override.
- **NFTs That Do Something**: a Cover Note is a transferable claim on a future payout; the holder at settlement is paid.
- **Agent Economy**: agents that buy or sell GPU time can lock next week's price; the keeper and every action are plain transactions an agent can send.
- **Autonomous Business**: markets run from auction to settlement without an operator; anyone can request observations, clear, settle and redeem.
- **Privacy & Private AI**: bids are sealed until the reveal window, and prices come from a pinned program run by OutLayer instead of a reporter.

## Run the checks

Requirements: Linux x86_64/aarch64 or macOS arm64 (near-sandbox has no Windows build; on Windows use WSL), Rust (rustup) with targets `wasm32-unknown-unknown` and `wasm32-wasip2`, `cargo-near` 0.23, Node 20+. Optional: Docker (reproducible oracle build) and OutLayer's test runner (`make oracle-runner`, several minutes once).

```bash
make vendor          # clone OutLayer at the pinned commit (build script + test runner)
make verify          # tests, builds, oracle, read-only testnet checks; prints a table
make demo-scripted   # replay testnet market 2 in the sandbox
make oracle-repro    # build oracle/ twice in OutLayer's pinned compiler image; prints the sha256
```

`make verify` reports PASS, FAIL, SKIP (a tool is missing: Docker or the OutLayer runner) or PENDING (the contract's full-access key, removed only after the owner approves). Every claim in this README has a row in [CLAIMS.md](CLAIMS.md) with a test or a testnet transaction.

## Limitations

- **Testnet only.** fUSD is a test token with a public faucet. No mainnet contracts.
- **Commit-reveal reveals bids after the commit window.** Bids are hidden while bidding, but the reveal makes them public before clearing. Clearing inside an enclave is not built.
- **Price sources.** Five public APIs (Akash, Vast.ai, RunPod, Shadeform, Verda). Shadeform aggregates other clouds, including Verda, so they are not fully independent. The rule per source is in [FACTS.md](FACTS.md) section 4. A source can change its API.
- **OutLayer trust.** OutLayer's governance still runs on keys its operators hold (near-outlayer `docs/NON_CUSTODIAL.md`). The operator can also refuse service; a market without enough observations settles void.
- **1Click quotes only.** No live cross-chain swap; quotes are mainnet prices. 1Click's quote endpoint timed out during the build; the page then shows the recorded quote, labelled "recorded".
- **Owner creates markets.** `create_market` is owner-only. The owner cannot change prices, payouts or settlement. The contract still has its full-access key (keyless step pending), and it has a one-time `migrate` method used for the Cover Note upgrade.
- **Guest sponsor.** The rate limit (3 accounts per IP per hour) is kept in serverless memory, so it is best-effort.
- **Not built.** Agent auto-hedging, the token price market, wallet login, Rhea secondary trading and mainnet payments (listed as extras in CLAUDE.md).

## Repository

`contracts/flatrate` (markets, auction, claims, observations, settlement, Cover Notes), `contracts/fusd` (test stablecoin), `contracts/mock-outlayer` (sandbox only, never deployed), `shared/` (payoff, median, auction clearing), `oracle/` (OutLayer component), `web/` (site and `web/api` serverless functions), `scripts/` (keeper, demo, verify), `fixtures/` (recorded API responses and the recorded market). Build log with real output: [PROGRESS.md](PROGRESS.md). Sources for every external fact: [FACTS.md](FACTS.md). Open items: [BLOCKERS.md](BLOCKERS.md).

Built at the NEAR Hackathon.
