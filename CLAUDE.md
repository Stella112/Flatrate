# Flatrate: Claude Code build prompt

Nov 12, 2026 · @maris

## How to use this prompt

This document is the complete instruction set for building Flatrate with Claude Code. Everything below this section is written to Claude Code.

Setup, done once by Maris:

- Export this doc as Markdown.
- Create an empty public GitHub repository `Stella112/flatrate` and clone it.
- Save the export at the repository root as `CLAUDE.md`.
- Open Claude Code in that folder and send: `Read CLAUDE.md in full. Then start Phase 0 and stop at its report.`
- After each phase report, reply `continue` or give corrections.

Repository layout Claude Code must create:

```
flatrate/
├── CLAUDE.md              # this prompt (do not edit except the Status block)
├── PROGRESS.md            # phase log: what was done, commands run, real output
├── BLOCKERS.md            # anything unverified or failing, with evidence
├── FACTS.md               # every external fact used, with its source link or file path
├── README.md              # for judges (written in Phase 6)
├── AGENTS.md              # for AI reviewers (written in Phase 6)
├── CLAIMS.md              # claim -> test or transaction that proves it
├── Makefile               # make verify, make test, make deploy-testnet, make demo
├── contracts/
│   ├── flatrate/          # main contract: markets, claims, auction, settlement, Cover Note NFT
│   ├── fusd/              # test stablecoin (NEP-141), testnet only
│   └── mock-outlayer/     # sandbox-only stand-in for outlayer.testnet, never deployed
├── oracle/                # OutLayer WASI component: fetch prices, take median
├── shared/                # Rust crate: payoff math and auction clearing, used by contract and tests
├── web/                   # Vite + React + TypeScript frontend
├── api/                   # serverless functions: guest account sponsor, 1Click quote proxy
├── scripts/               # deploy, keeper (price updates), demo, verify helpers
└── fixtures/              # recorded real API responses for scripted demo mode
```

Status block (Claude Code updates only this block in CLAUDE.md, at the end of each phase):

```
STATUS: phase=5 state=report-sent last_commit=see-git-log blockers=see-BLOCKERS.md
```

## Your role and mandate

You are Claude Code, acting as the sole engineer on Flatrate for Maris, a solo builder entering the NEAR Hackathon. You write, build, test and deploy the code. Maris makes product decisions and supplies accounts and keys. Your job is a working, verifiable testnet build by the deadline, not the largest feature list.

The entry will be checked by Claude Code and Codex acting as judges. They clone the repository, run the commands in README.md and AGENTS.md, and compare every claim against what actually runs. Anything you describe that does not run costs the entry more than leaving it out.

### Rule 1: never invent facts

Do not guess an account name, contract method, argument shape, crate or package version, API endpoint, model ID or price. Every such fact must come from one of these sources, in this order:

1. This document.
2. Source code or docs you cloned or fetched this session (paths and URLs are listed in the Resources section).
3. A command you ran whose output shows the fact.

Record each external fact in `FACTS.md` with its source (URL, file path and line, or command plus output).

If you cannot find a fact from those sources, write it in `BLOCKERS.md` and stop to ask Maris. Do not write placeholder code that pretends it works.

Sections marked UNVERIFIED in this document are my best information, not facts. Check them before use.

### Rule 2: stay in scope

- Build only what the Scope section lists as core, in the phase order given.
- Do not start an extra until every core acceptance check passes and Maris replies `go extras`.
- Do not add features, pages, tokens, chains, dependencies or refactors that are not asked for. If you think something is missing, write it in `BLOCKERS.md` as a suggestion and carry on.
- Do not rename the project, its contracts or its concepts.

### Rule 3: nothing fake is presented as real

- A mock, stub, fixture or hard-coded value must be named with `mock` or `fixture` in its file or symbol name, and must be labelled in the UI and README wherever it appears.
- `contracts/mock-outlayer` exists only for sandbox tests. It must never be deployed to testnet or referenced by the frontend.
- Scripted demo mode replays recorded real responses from `fixtures/` and shows a visible "Scripted demo" badge.
- Never write a README or AGENTS.md claim that does not have a row in `CLAIMS.md` pointing to a passing test or a real testnet transaction.

### Rule 4: prove each step

- Work in small steps: change, build, test, commit.
- A phase is done only when its acceptance commands pass. Paste the command and its real output (trimmed to the relevant lines) into `PROGRESS.md`.
- Never delete, skip or weaken a test to make it pass. Never edit an acceptance check. If one is wrong, say why in `BLOCKERS.md` and ask.
- After two failed attempts at the same error, stop and report the error, what you tried, and your best explanation.

### Rule 5: safety with money and keys

- Testnet only. Never create, import or use a mainnet key. Never send a mainnet transaction. The only exception is the optional mainnet payment extra, and only after Maris writes `approve mainnet payment` in chat.
- Never commit a private key, seed phrase, API key or `.env` file. Provide `.env.example` with names only.
- Keys live in `~/.near-credentials`, `.env` (gitignored), OutLayer secrets, or the hosting provider's environment settings.
- Do not delete the contract's full-access keys until Phase 6, and only after Maris writes `go keyless`.

### Stop and ask Maris when

- A fact in Rule 1 cannot be verified.
- A core feature cannot work as described (for example, OutLayer on testnet cannot fetch prices).
- An action costs real money or is hard to undo.
- A phase will clearly not finish in its time box.

### Phase report format

End every phase with this report in chat, and nothing longer:

```
PHASE <n> REPORT
Done: <one line per acceptance check, with PASS or FAIL>
Evidence: <PROGRESS.md section, tx hashes, deployed account names>
Changed from plan: <none, or what and why>
Blockers: <none, or BLOCKERS.md items>
Need from Maris: <none, or exact items>
Next: Phase <n+1>: <title>
```

## Project brief

Flatrate lets AI teams and agents lock next week's GPU price. A buyer of compute takes a long claim that pays more when the price rises. A seller of compute takes the short claim. Settlement uses an attested median of public GPU prices, fetched inside an OutLayer enclave, so no operator can set the price.

One market, end to end:

1. A market is created for one index (H100 price per GPU-hour), one period, a floor and a cap.
2. A sealed opening auction finds the opening price. Bids are hidden until the auction closes.
3. Matched bids mint pairs of claims. One dollar of collateral backs one long plus one short claim.
4. During the period, an OutLayer component fetches prices from several providers and posts the median on-chain.
5. After the period, anyone can settle. Long pays `clamp((avg - floor) / (cap - floor), 0, 1)` dollars per claim. Short pays the rest.
6. Claims can be wrapped in a Cover Note NFT, which pays whoever holds it.

Hackathon facts (from the official rules, mirrored in ebenbaruk/near-memecoin `Docs/sources/HACKATHON.md`):

- Development closes 14 November 2026, 23:59 UTC. The repository must be public and the demo live by then.
- Judging runs 16 to 18 November. Keep the hosted demo up until at least 25 November.
- Testnet-only projects are eligible.
- Maris submits two projects. Each README must name the other: Facedown (`Stella112/facedown`).
- README is read by machines first: plain steps in the form "do X, see Y", no marketing.
- The demo post on X must say it was built at the NEAR Hackathon, tag @NEARProtocol and use #NEARHackathon.
- Tracks to name in the README: Unstoppable DeFi; NFTs That Do Something; Agent Economy; Autonomous Business; Privacy & Private AI.

Time box. From the moment Phase 0 starts, there are about 48 hours. The phases below carry hour budgets. If a phase runs over by half its budget, stop and report.

## Architecture

Build exactly these components. Names in `code` are fixed unless a verified fact forces a change, which you then record in `PROGRESS.md`.

| Component | What it is | Deployed as |
|---|---|---|
| `contracts/fusd` | Test stablecoin, NEP-141, 6 decimals, public `faucet()` capped at 1,000 fUSD per call | `fusd.<root>.testnet` |
| `contracts/flatrate` | Markets, claims ledger, sealed auction, observations, settlement, Cover Note NFT (NEP-171) | `flatrate.<root>.testnet` |
| `oracle/` | OutLayer WASI component (Rust, `wasm32-wasip2`): fetch prices, return the median | OutLayer project `<root>.testnet/flatrate-oracle` |
| `shared/` | Pure Rust: payoff math, auction clearing, median | Compiled into the contract and tests |
| `web/` | Frontend | Vercel (or Netlify), static |
| `api/` | Guest account sponsor; 1Click and Aurora quote proxy | Vercel serverless functions |
| `scripts/keeper` | Requests price observations on a timer | Run by Maris or a hosted cron; anyone can run it |

`<root>` is the testnet account Maris provides (see Resources).

All amounts are integers. fUSD uses 6 decimals, so $1 = `1_000_000`. Prices are stored in micro-USD per unit (per GPU-hour). Auction prices are in whole cents from 1 to 99.

### Market

```rust
pub struct Market {
    pub id: u64,
    pub index: IndexId,            // H100_HOURLY (core) | TOKEN_PRICE (extra)
    pub floor_micros: u64,         // e.g. 1_000_000 = $1.00/hr; set from Phase 0 observed prices
    pub cap_micros: u64,           // must be > floor
    pub auction_commit_end: u64,   // ns timestamps
    pub auction_reveal_end: u64,
    pub period_start: u64,
    pub period_end: u64,
    pub min_observations: u32,     // demo: 3; production: 7 per week
    pub void_after: u64,           // if still unsettled after this, settle as void
    pub status: MarketStatus,      // Auction | Open | Settled{long_payout_micros} | Void
    pub observations: Vec<Observation>,
}
pub struct Observation { pub price_micros: u64, pub sources_ok: u8, pub at: u64, pub outlayer_request: String }
```

`create_market` is owner-only before the contract goes keyless. Create one demo market with short periods (for example a 20-minute auction and a 30-minute period) and one weekly market.

### Claims ledger

- Balances keyed by `(market_id, Side, AccountId)`, where `Side` is `Long` or `Short`.
- `mint_pair(market_id, amount)` via `ft_transfer_call` from fUSD: each $1 mints one Long and one Short to the sender. Allowed while the market is `Open`.
- `redeem_pair(market_id, amount)`: burn one Long plus one Short, return $1. Allowed until settlement.
- `claim_transfer(receiver_id, market_id, side, amount)` and view `claim_balance_of(account_id, market_id, side)`.
- `redeem(market_id)` after settlement: pays `balance x payout` for each side held, then zeroes the balances.

### Payoff

Implement once in `shared/` and use it everywhere:

```
avg     = integer mean of observation prices in the period
long    = clamp((avg - floor) * 1_000_000 / (cap - floor), 0, 1_000_000)   // micro-USD per Long claim
short   = 1_000_000 - long
void    -> long = short = 500_000
```

Round down for Long and give the remainder to Short, so the contract never pays out more than it holds. Add a test that, for random inputs, `long + short == 1_000_000`.

### Sealed opening auction (core: commit and reveal)

- Commit (until `auction_commit_end`): `commit_bid(market_id, hash)` via `ft_transfer_call` with an escrow deposit `D`. `hash = sha256(borsh(market_id, side, qty, limit_cents, salt))`. Bidders may deposit more than they need, so the deposit does not reveal the bid.
- Reveal (until `auction_reveal_end`): `reveal_bid(market_id, side, qty, limit_cents, salt)`. Valid only if the hash matches and `qty x limit_cents x 10_000 <= D`.
- Clear: anyone calls `clear_auction(market_id)` after reveal ends.
  - For each price `P` in 1..=99: `L(P)` = total qty of Long bids with `limit >= P`; `S(P)` = total qty of Short bids with `limit >= 100 - P`; `matched(P) = min(L(P), S(P))`.
  - Choose the `P` with the most matched; on a tie, the `P` closest to 50; then the lower `P`.
  - Fill bids by better limit first, then earlier commit. The marginal bid may be filled partly.
  - Each matched pair: Long buyer pays `P` cents, Short buyer pays `100 - P` cents, together $1, which mints one pair.
  - Refund every unused escrow, including unrevealed bids. The market moves to `Open` with the opening price recorded.
- Cap bids at 200 per auction so `clear_auction` fits in gas. Test with 200 bids.

The stronger version, where bids are encrypted to an OutLayer component and cleared inside the enclave so no bid is ever public, is an extra.

### Price observations through OutLayer

- `request_observation(market_id)` is public and payable. The caller attaches the OutLayer deposit. The contract rejects calls outside the market period.
- The contract calls `request_execution` on `outlayer.testnet` with `source = Project { project_id: "<root>.testnet/flatrate-oracle", version_key: <pinned> }`. The pinned version is stored in contract state and shown on the Proof page.
- The result returns through NEAR's yield and resume flow to a callback, `on_observation`. Read `contract/README.md` and `wasi-examples/PROXY_CONTRACTS_TUTORIAL.md` in the OutLayer repository for the exact call and callback shape before writing this.
- The callback accepts a result only if its predecessor is the OutLayer contract and it matches a pending request id stored by `request_observation`. Any other caller fails with `ERR_NOT_OUTLAYER`.
- Validate the output: at least 3 sources succeeded, price above 0 and below `10 x cap`. Otherwise record nothing and emit an event saying why.

### Oracle component (`oracle/`)

- Rust, target `wasm32-wasip2` (HTTP needs Preview 2), crates `outlayer` and `wasi-http-client`. Confirm versions from the OutLayer repository's examples and write them in `FACTS.md`.
- Input JSON: `{ "index": "H100_HOURLY", "sources": ["akash", "..."] }`.
- For each source: one HTTP request, parse the H100 on-demand hourly price, convert to micro-USD per GPU-hour. Write the exact selection rule per source (which GPU model string, which field) in `FACTS.md`.
- Output JSON: `{ "index": ..., "price_micros": <median>, "sources": [{"name", "price_micros" | null, "error" | null}], "fetched_at": <unix seconds> }`.
- API keys, if a source needs one, come from OutLayer secrets. Never put a key in the code or the input.
- Test locally with OutLayer's `wasi-examples/wasi-test-runner` before uploading.

### Settlement

- `settle(market_id)`: public, after `period_end`. If observations `>= min_observations`, compute the average and the Long payout, set `Settled`. Otherwise do nothing until `void_after`, then set `Void`.
- Settlement is final. There is no admin override.

### Cover Note NFT (NEP-171, in the same contract)

- Use `near-contract-standards` for NEP-171 core, enumeration (NEP-181) and metadata (NEP-177).
- `wrap_note(market_id, side, amount)`: moves the caller's claims into the note and mints a token. Metadata title, for example: `Cover Note #12: 100 Long, H100, period ending 2026-11-21`. Extra fields: market, side, amount, floor, cap, period end.
- The note can be transferred like any NFT. The claims move with it.
- `redeem_note(token_id)` after settlement: pays the current owner, then burns the token.
- `unwrap_note(token_id)` before settlement: returns the claims to the owner and burns the token.

### Frontend (`web/`)

Vite, React, TypeScript, `near-api-js`. Pages:

- Markets: list with status, floor, cap, latest median, time left.
- Market: price chart from observations; auction commit and reveal forms (the salt is saved in browser storage and shown for backup); mint, redeem, transfer claims; settle button.
- My notes: Cover Notes owned, wrap, unwrap, redeem, transfer.
- Proof: every observation with its OutLayer request id, transaction link, and per-source prices; the pinned oracle version; attack buttons (see Tests).
- Pay from any chain: a live quote from the NEAR Intents 1Click API with `dry: true`, showing what it would cost to pay in from another chain. Label it "quote only on testnet".

No wallet needed. On first visit the browser creates a key pair. `api/sponsor` creates `the implicit account for that key (its hex public key)` by sending it 0.5 NEAR, then gives it 100 fUSD, and rate-limits by IP. The private key stays in browser storage. Wallet login with NEAR Wallet Selector is an extra.

### What is deliberately not in this architecture

No leverage, no liquidations, no admin price setting, no upgrade path after keyless, no token of our own beyond the test stablecoin.

## Build order

Work through the phases in order. Commit at the end of each phase with a message starting `phase N:`, push, update the Status block, and send the phase report. Phase 0 always stops for Maris's reply. Later phases continue automatically unless a stop condition from your mandate applies.

### Phase 0: verify everything (3 hours) — stop for review

No product code in this phase. Its output is `FACTS.md`, `BLOCKERS.md` and a go or no-go.

1. Tool check: record versions of `rustc`, `cargo`, `cargo-near`, `near` (near-cli-rs), `node`, `pnpm` or `npm`. Install what is missing with the official installers listed in Resources.
2. Clone `https://github.com/fastnear/near-outlayer` into `vendor/near-outlayer` (gitignored). Read `README.md`, `contract/README.md`, `API.md`, `VRF.md`, `docs/CLI.md`, `wasi-examples/WASI_TUTORIAL.md`, `wasi-examples/PROXY_CONTRACTS_TUTORIAL.md`, `wasi-examples/BEST_PRACTICES_OUTLAYER_NEAR.md`, and the `test-secrets-example` and `weather-example` folders. Record in `FACTS.md`: the `request_execution` arguments, how the callback reaches your contract, deposit estimation (`estimate_execution_cost`), how secrets are referenced, the `outlayer` and `wasi-http-client` crate versions.
3. Confirm a testnet RPC endpoint responds (start with `https://rpc.testnet.fastnear.com`; alternatives at docs.near.org RPC providers). Record it.
4. Confirm Maris's root account exists and has balance: `near account view-account-summary <root>.testnet network-config testnet now`.
5. OutLayer smoke test on testnet: build and run the repository's `echo-example` or `weather-example` through the OutLayer CLI against `https://testnet-api.outlayer.ai`, and once through a minimal contract calling `outlayer.testnet` `request_execution`. Record the transaction hash, latency, and cost.
6. Price sources: call each candidate (see Resources) once with `curl`. For each, record: works without key or needs key, the H100 field, the price returned. Keep only sources that work. At least 3 must work, or stop. Save the raw responses to `fixtures/prices/<source>.json`.
7. 1Click: one `POST /v0/quote` with `dry: true` (for example USDC on Base to USDC on NEAR; it returns mainnet prices and sends nothing). Save the response to `fixtures/1click-quote.json`.
8. If Maris has supplied an Aurora Intents API key: one quote request against the Aurora Intents API. Save it.
9. Propose `floor_micros` and `cap_micros` for the H100 market from today's median: floor about 0.6 x median, cap about 1.4 x median, rounded to 10 cents.

Acceptance: `FACTS.md` has every item above with a source; `fixtures/` has the raw responses; one successful OutLayer testnet transaction hash is recorded. Then send the report and wait.

### Phase 1: contracts core (6 hours)

- `shared/`: payoff, median, auction clearing as pure functions with unit tests, including property tests for `long + short == 1_000_000` and for clearing never minting more than escrow covers.
- `contracts/fusd`: NEP-141 with `faucet()`. Storage registration per NEP-145.
- `contracts/flatrate`: markets, claims ledger, `mint_pair`, `redeem_pair`, `claim_transfer`, `settle`, `redeem`, void path. Observations are written through an internal function only reachable from the OutLayer callback.
- `contracts/mock-outlayer` and sandbox tests with `near-workspaces`: create market, mint, record observations through the mock, settle, redeem; void path; a non-OutLayer caller calling `on_observation` fails.

Acceptance: `make test` passes; `cargo near build` succeeds for both contracts; test output pasted in `PROGRESS.md`.

### Phase 2: sealed opening auction (5 hours)

- `commit_bid` through `ft_transfer_call`, `reveal_bid`, `clear_auction`, refunds.
- Sandbox tests: a simple two-bid match; a tie-break; partial fill; an unrevealed bid refunded; a wrong salt rejected; a reveal after the deadline rejected; 200 bids clear within gas.

Acceptance: all auction tests pass; gas used by the 200-bid clear recorded in `PROGRESS.md`.

### Phase 3: real prices through OutLayer (6 hours)

- Build `oracle/` for `wasm32-wasip2`. Run it locally with the OutLayer test runner against live sources. Then against `fixtures/` to confirm parsing.
- Create the OutLayer project `<root>.testnet/flatrate-oracle`, upload, store secrets if any source needs a key.
- Deploy `fusd` and `flatrate` to testnet. Pin the oracle version. Create the demo market and the weekly market.
- `scripts/keeper`: calls `request_observation` every N minutes for open markets.
- Run the demo market to settlement on testnet: at least 3 real observations, settle, redeem.

Acceptance: testnet transaction hashes for 3 observations, the settlement and a redemption, recorded in `PROGRESS.md` and `CLAIMS.md`.

### Phase 4: Cover Note NFT (3 hours)

- NEP-171, NEP-177, NEP-181 via `near-contract-standards`. `wrap_note`, `unwrap_note`, `redeem_note`.
- Sandbox tests: wrap, transfer to another account, the new owner redeems after settlement; the old owner cannot.
- Redeploy to testnet. One real wrap, transfer and redeem.

Acceptance: tests pass; three testnet transaction hashes recorded.

### Phase 5: frontend, sponsor and hosting (8 hours)

- Pages listed in Architecture. Guest accounts through `api/sponsor`.
- 1Click dry-run quote panel through `api/quote` (the server adds no secrets on testnet; if Maris supplies a 1Click JWT, it is sent server-side only).
- Scripted demo mode at `/?demo=scripted`: replays `fixtures/` with a visible badge.
- Deploy to Vercel or Netlify. Record the public URL.

Acceptance: from a clean browser profile, the full flow works on the hosted URL: guest account, faucet, commit, reveal, clear (on the demo market), mint, wrap, transfer, settle, redeem. Screenshots saved to `docs/screens/`.

### Phase 6: proof layer and submission (5 hours)

- `make verify`, `CLAIMS.md`, `AGENTS.md`, `README.md` as specified in the Tests and README sections.
- Attack buttons wired on the Proof page.
- Only after Maris writes `go keyless`: delete the full-access keys on `flatrate.<root>.testnet` and record the `view-account-keys` output in `CLAIMS.md`.
- Write `docs/demo-script.md`: a 2-minute screen recording plan following the Demo section of the Flatrate description.

Acceptance: `make verify` passes from a fresh clone on a machine with no NEAR credentials; README steps followed literally succeed.

### Extras, only after `go extras`, in this order

1. Agent auto-hedging: a script holding a function-call access key limited to the flatrate contract, which bids in auctions from a usage forecast file.
2. Token price market: an OpenRouter-based index (see Resources).
3. Enclave-cleared auction: bids encrypted to an OutLayer component, cleared inside it.
4. Secondary trading on Rhea (only if a Rhea testnet contract is confirmed live in Phase 0).
5. One real mainnet pay-in through NEAR Intents and a payout through Aurora Intents, only after `approve mainnet payment`.
6. Wallet login with NEAR Wallet Selector.

## Resources and requirements

### From Maris before Phase 0

- [ ] Root testnet account (for example `flatrate-maris.testnet`), its credentials in `~/.near-credentials/testnet/`, funded from the testnet faucet. Name it in chat.
- [ ] Public GitHub repo `Stella112/flatrate`, cloned, with push access from this machine.
- [ ] Vercel or Netlify account linked to the repo, for the frontend and `api/`.
- [ ] Sponsor key: a second testnet account (for example `sponsor.flatrate-maris.testnet`) whose key goes only into the hosting provider's environment settings as `SPONSOR_ACCOUNT_ID` and `SPONSOR_PRIVATE_KEY`.
- [ ] Optional: Aurora Intents API key from portal.intents.aurora.dev, as `AURORA_INTENTS_API_KEY`.
- [ ] Optional: 1Click JWT, as `ONECLICK_JWT`. Without it, quotes still work; live swaps carry a 0.1% fee.
- [ ] Optional: API keys for any price source that needs one, stored only as OutLayer secrets.

### Verified facts (checked on 12 November 2026)

| Fact | Value | Source |
|---|---|---|
| OutLayer contract | `outlayer.testnet` (testnet), `outlayer.near` (mainnet) | near-outlayer `README.md` |
| OutLayer HTTPS API | `https://testnet-api.outlayer.ai`, `POST /call/{owner}/{project}` | near-outlayer `README.md` |
| OutLayer docs for AI tools | `https://app.outlayer.ai/llms-full.txt` | near-outlayer `README.md` |
| Contract call | `request_execution(source, resource_limits, input_data, secrets_ref, response_format, payer_account_id, params)`; result by yield and resume; pass `resource_limits: {}` or nothing runs | near-outlayer `README.md`, `contract/README.md` |
| Deposit | `estimate_execution_cost(resource_limits, input_len)` view, in yoctoNEAR | near-outlayer `contract/README.md` |
| Source formats | `{"Project": {"project_id", "version_key"}}` or `{"WasmUrl": {"url", "hash", "build_target"}}` | near-outlayer `contract/README.md` |
| WASI | Rust SDK crate `outlayer` (0.2); HTTP needs `wasm32-wasip2` and `wasi-http-client` | near-outlayer `VRF.md`, `WASI_TUTORIAL.md` |
| Trust caveat | OutLayer governance still runs on keys it holds | near-outlayer `docs/NON_CUSTODIAL.md` |
| NEAR Intents 1Click | Base `https://1click.chaindefuser.com`; `GET /v0/tokens`, `POST /v0/quote` (`dry: true` for quotes), `GET /v0/status`; JWT avoids a 0.1% fee | docs.near-intents.org 1Click API |
| 1Click has no testnet | Quotes and swaps are mainnet only | Research earlier in this project |
| Aurora Intents API | Quote endpoint documented at docs.intents.aurora.dev; key from portal.intents.aurora.dev | Aurora docs |
| Chain signatures | MPC contract `v1.signer-prod.testnet`; JS library chainsig.js | docs.near.org chain signatures |
| NEAR Rust SDK | `near-sdk` 5.x, `cargo-near`, `near-workspaces` for sandbox tests | docs.rs near-sdk, near-workspaces-rs |

### UNVERIFIED: check in Phase 0

- Price sources for H100 per hour. Candidates: Akash `https://console-api.akash.network/v1/gpu-prices`; Vast.ai public offer search; RunPod GraphQL `https://api.runpod.io/graphql` (may need a key); Shadeform `https://api.shadeform.ai/v1/instances/types` (needs a key); Verda (formerly DataCrunch) instance types API; Ornn. Use only the ones that work.
- Token prices (extra). OpenRouter `https://openrouter.ai/api/v1/models` lists per-model prices; a per-provider endpoint list may exist under `/api/v1/models/{author}/{slug}/endpoints`.
- Rhea on testnet (extra only): `ref-finance-101.testnet` may be the testnet contract. Check with a view call before using.
- OutLayer testnet latency and cost per call. Measure.
- Testnet RPC. `https://rpc.testnet.fastnear.com` is the first choice; confirm.

### Tools to install

- Rust via rustup, with targets `wasm32-unknown-unknown` and `wasm32-wasip2`.
- `cargo-near` and `near-cli-rs`, from their official GitHub release installers.
- Node 20 or later, with pnpm or npm.
- Vercel CLI or Netlify CLI.

### Reading list for Claude Code

- OutLayer source and docs: `vendor/near-outlayer` (cloned in Phase 0), plus `https://app.outlayer.ai/llms-full.txt` if reachable.
- NEAR smart contracts: docs.near.org quickstart, near-sdk-rs, `near-contract-standards` for NEP-141 and NEP-171.
- Flatrate description doc (product intent): https://claude.ai/artifact/FdtYGwJkuJcGs1PFtqycHq. Where it differs from this build prompt, this prompt wins.

## Tests and proof layer

The judges are Claude Code and Codex running your commands. Build for them.

### `make verify`

One command that needs no wallet, no secret and no network writes. It must:

1. Run all `shared/` unit and property tests.
2. Build both contracts with `cargo near build` and run all sandbox tests.
3. Run the oracle component against `fixtures/prices/` with the OutLayer test runner and check the median.
4. Read the deployed testnet contracts through public RPC (view calls only) and check: the pinned oracle version matches the hash of `oracle/` built from this commit; each settled market's payout equals the payoff recomputed from its stored observations; `flatrate.<root>.testnet` has no full-access key (after keyless).
5. Print a table: check, PASS or FAIL, evidence.

It must finish in under 10 minutes on a normal laptop. Exit non-zero on any FAIL.

### Required sandbox tests

| Area | Test |
|---|---|
| Payoff | Floor, cap, mid, below floor, above cap, void; `long + short == 1_000_000` for random inputs |
| Ledger | Mint, transfer, redeem pair; redeem after settle; double redeem pays nothing |
| Auction | Two-bid match; tie-break; partial fill; unrevealed refund; wrong salt; late reveal; 200 bids in gas |
| Oracle callback | Non-OutLayer caller rejected (`ERR_NOT_OUTLAYER`); unknown request id rejected; fewer than 3 sources rejected; out-of-range price rejected |
| Settlement | Too early refused; too few observations waits, then voids; second settle refused |
| Cover Note | Wrap, transfer, new owner redeems, old owner cannot; unwrap before settle |

### Attack buttons (Proof page)

Each sends a real testnet transaction from the guest account and shows the on-chain failure message:

- Fake price: calls `on_observation` directly with a made-up price. Expected: `ERR_NOT_OUTLAYER`.
- Early settlement: calls `settle` on an open market. Expected: `ERR_PERIOD_NOT_ENDED`.
- Replay: calls `on_observation` with an old request id. Expected: `ERR_UNKNOWN_REQUEST`.
- Steal a note: tries to redeem a Cover Note the guest does not own. Expected: `ERR_NOT_OWNER`.

### `CLAIMS.md`

A table with one row per README claim: claim, how to check (test name or command), evidence (transaction hash with an explorer link, for example `https://testnet.nearblocks.io/txns/<hash>`). No claim without a row.

### `AGENTS.md`

For AI reviewers. Plain steps only:

```
1. Run: make verify        -> expect: all PASS, exit 0
2. Open: <hosted URL>/proof -> expect: observations with OutLayer request ids
3. Run: make demo-scripted  -> expect: full market lifecycle from fixtures, badge shown
4. Read: CLAIMS.md          -> each row has a test or tx
5. Read: README.md "Limitations"
```

### Scripted demo mode

`make demo-scripted` and `/?demo=scripted` replay a full market from `fixtures/` against the sandbox, with the same result every time and no keys. The UI shows a "Scripted demo" badge on every screen in this mode.

## README and submission

### README.md structure (write in Phase 6)

1. One paragraph: what Flatrate is and who it is for.
2. Try it: hosted URL; `make verify`; `make demo-scripted`.
3. How it works: the six steps from the Project brief, plus a short diagram.
4. Deployed on testnet: every account name and the pinned oracle version.
5. What NEAR does here ("remove NEAR and it stops"): OutLayer attested price fetching; yield and resume callbacks; keyless contract; account-level access keys for agents; NEAR Intents quotes for paying in from other chains.
6. Tracks: the five track names, one line each on the fit.
7. Limitations: testnet only; test stablecoin; commit-reveal auction reveals bids after the auction closes; price sources and their count; OutLayer governance caveat from `docs/NON_CUSTODIAL.md`; 1Click quotes only, no live swap on testnet.
8. Other submission by the same builder: Facedown, `https://github.com/Stella112/facedown`.
9. Built at the NEAR Hackathon.

No marketing words. Every sentence that claims something must match a row in `CLAIMS.md`.

### Submission checklist (before 14 November 2026, 23:59 UTC)

- [ ] Repository public, default branch builds from a fresh clone.
- [ ] `make verify` passes from a fresh clone.
- [ ] Hosted demo live; will stay up until at least 25 November.
- [ ] README names Facedown.
- [ ] Demo video recorded from `docs/demo-script.md`.
- [ ] X post drafted: says it was built at the NEAR Hackathon, tags @NEARProtocol, uses #NEARHackathon. Maris posts it.
- [ ] Submission form filled by Maris.

## Out of scope (do not build)

- Mainnet deployment of contracts.
- Leverage, margin, liquidation, perpetual contracts.
- Any governance token or project token.
- Admin functions that change prices, payouts or settlements.
- Bridges you write yourself.
- Mobile app, email, notifications, analytics.
- Any feature not in the core or extras lists.
