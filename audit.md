# Flatrate audit

Date: 2026-11-14 (00:10 to about 09:30 UTC). Audited commit: `6724695` plus the changes listed under "Fixed in this audit". Network: NEAR testnet. Every transaction below is on `https://testnet.nearblocks.io/txns/<hash>`.

## Verdict

Every core feature in the product description runs on testnet. During this audit a fresh guest ran a whole market from the hosted site: account, roll, sealed bids, reveal, clearing, mint, Cover Note wrap and transfer, four attacks, settlement and redemption. The keeper and OutLayer handled the rest with no operator action. The read-only chain checks pass for both contracts.

Fixed during the audit: the Proof page could not show any transaction (rate limiting); the keeper filled only every other slot; the sponsor gave guests far more NEAR than they need; the guest button stayed visible after funding. Not built, with reasons in BLOCKERS.md B12 and B13: enclave-cleared auction, Rhea trading, real mainnet payments, cross-chain value transfer. Still needs Maris: the Aurora key, the sponsor top-up, the market 10 reveal window, and the submission steps.

## What was run

| Check | Where | Result |
|---|---|---|
| `make verify`, fresh clone of `6724695` | WSL Debian, near-sandbox 2.14.0 binary supplied through `NEAR_SANDBOX_BIN_PATH` | every row PASS (shared 18, contract unit 8, three builds, agent 5, oracle 5, token oracle 3) except the two "chain checks" rows: `TypeError: fetch failed` from WSL's network (see environment notes); Docker and OutLayer-runner rows SKIP |
| Read-only chain checks (`node scripts/verify-chain.mjs`) for both contracts | Windows, Node 26 | PASS (details below) |
| `make test` (shared, unit, oracle, sandbox suite) | WSL, fresh clone | PASS: shared 18, fusd 3, oracle 5 + 5, sandbox `test result: ok. 30 passed; 0 failed` in 983 s |
| `make demo-scripted` | WSL, fresh clone | PASS: `SCRIPTED DEMO PASS: sandbox replay equals testnet market 2` |
| `make test-agent`, `make agent-dry` | Windows | `pass 5`; dry run prints a Long 64 @ <= 53c bid for WEEKLY market 10 |
| `npm run build` in `web/` | Windows | builds; one warning from a dependency's direct `eval` (not ours) |
| Full lifecycle from the hosted site as a new guest | https://flatrate-gpu.vercel.app, DEMO market 14 | PASS (table below) |
| Four attack buttons | hosted Proof page | all blocked with the expected error |
| Hosted pages: Markets, Market, My notes, Proof, Pay, scripted demo, LLM tokens | hosted site | load with no console errors; findings below |
| Updated keeper, run locally against testnet | DEMO market 16 | PASS: rolled market 16 (`AHe1uyhmKNPXpaHAtCz3DTgC1Cyq22TF9dCS7Gc6izqJ`), cleared it (`HHVSybnZdJqJGfa4oq6cyVcMBxbWX5MPstTwWYdkEjqg`), sent each `request_observation` in about 4 s instead of 2 minutes (`6At1j31tRJSimPgts2Kcm7FpPdZnnvovbCEFjFkbhY3p`, `AXbodL54tHDnBU7Jd8Anau8Q4GBKQ8Q1CebdkTYVorWZ`, `98dxus5hPBdENau1K1fMoXXXdS1tWeQTfDuofFfDU4Ek`, `APUm9jmd1PE2wjPsqpNPqDDTUtTSGHxNfwTWW9nudVc2`), settled it (`DAhPRAdiG5iAnMfWkwEDtkyfYjc9c2wV3upLQq1JKKZf`); with the old server keeper taking the other slots, 7 of 8 filled. It then rolled DEMO market 17 (`8dnmLJaP4qFGQcYpSoAE27xGb3tbzARmSnsb4oA6js8e`), which runs with no bids |

Environment notes (this machine, not the project): drive C: was full during the first verify attempt, which made WSL's disk read-only and corrupted the Rust incremental cache; `target/debug` was deleted to free space and the reruns used `CARGO_INCREMENTAL=0`. Inside WSL, Node's `fetch` and the near-sandbox download time out while `curl` works, so the two "chain checks" rows of `make verify` fail inside WSL with `TypeError: fetch failed`; the same script passes from Windows. The last complete fresh-clone `make verify` with no FAIL row is the one on the Linux VPS at commit `374f7d3` (PROGRESS.md "Final fresh clone after the extras").

### Read-only chain checks (Windows, 2026-11-14)

```
contract readable                        PASS  flatrate.flatrate-maris.testnet
OutLayer version 6c252d5280… exists      PASS
market 11/12/13 payout recomputed        PASS  (493750, 525000, 525000)
OutLayer attestation per recorded price  PASS  16 match pinned wasm + market input
contract has no full-access key          PASS  0 keys
[tokens] contract readable               PASS  tokens.flatrate-maris.testnet
[tokens] market 10 payout recomputed     PASS  8/8 slots, long 620000
[tokens] attestation per recorded price  PASS  8 match
[tokens] contract has no full-access key PENDING  1 full-access key (needs "go keyless")
pinned version = reproducible build      SKIP  needs Docker
```

## Live run: DEMO market 14 from the hosted site (2026-11-14)

A browser profile with no Flatrate state opened the site; the page made a key and the guest pressed every button itself. The keeper (server cron, function-call key) cleared, priced and settled.

| Step | Who | Tx | Result |
|---|---|---|---|
| Create guest account | sponsor | `5qajZs4nj7C435qzNKpfvgb1MFBTGc97juZsmbWTpQMU` | 0.5 NEAR + 100 fUSD (old amount) |
| Roll the next DEMO market | guest | `b1na9Fp1uPfsA9uNCXHjUhfpwiV9GuxmzfL3xw2deER` | market 14, band 2.30 / 5.50 |
| Commit Long 10 @ <= 55c | guest | `D1wYDdsFxAvZBQDi71aABastz1EhqpgFGueUVsUJuUyd` | escrow 5.5 fUSD |
| Commit Short 10 @ <= 50c | guest | `39VtpDPhcZGb1GqGhNGx1B3xd4JFwLbh6hBRV64jEarp` | escrow 5 fUSD |
| Reveal both | guest | `FhAdXpbHRvzKB27A1bkbLDLMcmWLAXoHx96v2k1K2m5a`, `2QgC6xcXTaWGJsx9mGWyGwRgac2s4WgB6ccH6rczu5ja` | |
| clear_auction | keeper | `55U5D4dL3Us271z2G753FfMeuSmwcz43AjXqWbWx1obj` | opened at 50c, 10 pairs |
| Request a price in a slot the keeper already asked for | guest | `Ew7kWj35g2Nv22mTih7jiahx8muXTUVd3W255XEhuaKH` | `ERR_SLOT_FILLED` (correct) |
| Withdraw refund | guest | `332ts4eC2hRb7bn5rWa6E2JtFphJqhd76gghMPYRunC8` | 0.5 fUSD |
| Mint 10 pairs | guest | `XwzvtkShNTcfMM1v5unKSphioUHkwRy9rj3aN6zo1tF` | 20 Long, 20 Short held |
| Wrap 10 Long in a Cover Note | guest | `5q5h8gr3kJJ2QhpNzGD15WnooX1nCRRApcGyTAZyYK7r` | note #1002 |
| Transfer the note to `holder.flatrate-maris.testnet` | guest | `EwkykuVvJtiKt4D5epHoVK6YyfmLXr8EVBx2pAgLZqKj` | |
| Prices, slots 1, 3, 5, 7 | keeper | `4QPz2do83vHvUaTzUZNRd7J3fpXMyx36Nf7GwxREKWvZ`, `8xsRqsP7RSA3ykT9pGDbNNRtrJhJwV6y29as3tBoYbgg`, `ECesZP1KA5H5y2V4NGKdvihh6nciT36gB44PtsW8Prnp`, `71RYAs2axdQUj6a5V6EWUjxnGrgJZzFWbfXxYcdm7RhS` | 3.27 (4 sources; Verda errored), 3.98, 3.98, 3.98 |
| settle | keeper | `8fJZ4Arej7sV1M3eAaDZ1WMRS7cMWpDp5XegXgCjBkWL` | average 3.8025, Long 0.469531 = (3.8025 − 2.30) / 3.20 |
| Redeem 10 Long + 20 Short | guest | `ATLeu9eCvFqGPymRuTgQhRvCJCmf3jPe1XaXMVJuB5ZJ` | 15.30469 fUSD = 10 × 0.469531 + 20 × 0.530469 |
| Redeem note #1002 | new holder | `CM8koJhHTfSEMZa6Cittd3phy1r4uME7VTn5s1Xjy4Et` | 4.69531 fUSD, note burned |

The Proof page showed all four prices of market 14 with OutLayer request ids 41618, 41625, 41633 (and the fourth), per-source prices, fees, and "wasm ✓ · input ✓ · output ✓". The guest spent 0.0059 NEAR on the whole run (0.5 → 0.494101662 NEAR).

### Attack buttons (fresh guest, market 14 open)

| Attack | Expected | Got | Tx |
|---|---|---|---|
| Fake price (`on_observation` from the guest) | `ERR_NOT_OUTLAYER` | `ERR_NOT_OUTLAYER` | `Zz7g8QDAUjPgxDWBH7frD3Caj5DeGXHMWoCAZrT6xAr` |
| Early settlement | `ERR_PERIOD_NOT_ENDED` | `ERR_PERIOD_NOT_ENDED` | `HjtBUE8Xt47PFWYsyRDVfmtp17ztUk2jCvGFWQJuzVWi` |
| Second price in the same slot | `ERR_SLOT_FILLED` | `ERR_SLOT_FILLED` | `UZQMS4qCsmmWTbbRoQ5iUTJVgiuG3xugmRnthbWCa4y` |
| Steal a note (`redeem_note` on note #1001, owned by holder) | `ERR_NOT_OWNER` | `ERR_NOT_OWNER` | `CEZfXPUxrpvP8xpmQWKAvFVNw7RkMehiR6zPsFbADtde` |

The same-slot button first said "No open market has a price for its current slot right now": it only fires when the current slot is already filled or requested, so it can need a retry a minute later. Expected behaviour, not a bug.

## Against the product description

| Description says | Status | Evidence |
|---|---|---|
| Attested price feed, at least three sources, median | Done: five sources (Akash, Vast.ai, RunPod, Shadeform, Verda); a result needs at least 3 | market 14 slot 1 recorded with 4 sources |
| One price per time slot | Done | attack `UZQMS4qC…`; sandbox `second_request_in_filled_slot_fails` |
| Long and short claims: mint, redeem, transfer | Done | market 14 mint and redeem; sandbox ledger tests |
| Sealed opening auction, one clearing price, refunds | Done | market 14 commit/reveal/clear/refund |
| Settlement on the period average, midpoint if too few prices | Done | market 14 settle; void path sandbox test and v1 market 0 |
| Cover Note NFT that pays its holder | Done | note #1002 wrapped, transferred, redeemed by the new holder |
| Keyless contract, anyone rolls the next market, band from the last average | Done | 0 access keys; market 14 rolled by a guest, band 2.30 / 5.50 from 3.98 |
| No-wallet hosted demo | Done | sponsor tx `5qajZs4n…` |
| Proof page and attack buttons | Done (fixed in this audit, see below) | |
| TEE: Proof page shows OutLayer's attestation data | Done: wasm, input and output hashes checked per price; the TDX quote itself is not verified (stated in README) | |
| NEAR Intents and Aurora quotes side by side | Partly: 1Click live; Aurora shows "Not configured on this deployment" because `AURORA_INTENTS_API_KEY` is not set in Vercel | `GET /api/quote` → `"aurora": false` |
| Verify command, claims table, scripted demo | Done; `make verify` passes on Linux with working network (see environment notes) | |
| Demo market of 1-minute slots settling in about 15 minutes | Done: 3 + 2 minute auction, 8 one-minute slots, 13 minutes | market 14 |
| One weekly market live through judging | WEEKLY market 10: two sealed bids committed; must be revealed 2026-11-14 11:32 to 23:32 UTC (`scripts/reveal-bids.sh`), then 28 six-hour slots to 2026-11-21 | |
| Tracks: Unstoppable DeFi, NFTs That Do Something, Privacy & Private AI; Agent Economy and Autonomous Business if the agent ships | Done: README now names all five | |
| Extra 1, agent auto-hedging | Built with one change: commits need a full-access key (BLOCKERS.md B11) | DEMO market 13 |
| Extra 2, private (enclave-cleared) auction | Not built (B12) | |
| Extra 3, model token price market | Built: `tokens.flatrate-maris.testnet`, DEMO market 10 settled and redeemed | |
| Extra 4, cross-chain pay-in and payout | Reduced to chain signatures on Base Sepolia (B13) | |
| Extra 5, real mainnet payment | Not built: needs "approve mainnet payment" and a funded mainnet wallet (B12) | |
| Extra 6, Rhea pool | Not built: claims are not NEP-141 tokens and the contract is keyless (B12) | |
| Wallet login | Built; no wallet-signed transaction yet (README Limitations) | |

## Fixed in this audit

1. **Proof page showed "not found" for every transaction** (commit `6724695`, deployed and checked). The page sent all observation lookups to the archival RPC at once; the node answered 429 without CORS headers, so each lookup failed silently. Now two at a time, with retries and a fallback to the regular RPC. After the deploy, all 16 rows (and market 14's four) show the tx, per-source prices, fee and attestation checks. It still takes about a minute to fill.
2. **Keeper filled only every other slot of a DEMO market.** `scripts/keeper.mjs` waited for OutLayer's answer (one to two minutes) inside a one-minute cron guarded by `flock`, so the next run was skipped. Markets 14 and 15 got 4 and 5 of 8 slots. `request_observation` is now signed with near-cli and broadcast with `wait_until: INCLUDED`; a rejected result frees the slot and the next pass asks again. Tested on DEMO market 16 (row "Updated keeper" above). Takes effect for the hosted markets once the VPS runs this commit.
3. **Sponsor gave each guest 0.5 NEAR.** A whole lifecycle used 0.0059 NEAR, so the sponsor's 8.47 NEAR covered about 16 guests. It now gives 0.1 NEAR (about 80 guests on the same balance). README, CLAIMS and the site text say 0.1.
4. **"Create guest account" stayed visible after funding** until a reload. The button now hides on success, and the balance is re-read at 2.5, 6 and 12 seconds.

Fixes 3 and 4 were built and type-checked locally (`npm run build`), and the 0.1 NEAR text was checked on the local dev server; they reach the hosted site when this commit is deployed, and have not yet been checked there.
5. **README named four tracks**; Autonomous Business added, with a CLAIMS row (market 14).

## Open items and risks

| # | Item | Impact | Who |
|---|---|---|---|
| 1 | WEEKLY market 10 bids must be revealed between 2026-11-14 11:32 and 23:32 UTC | Without the reveal the weekly market opens with no pairs during judging | `scripts/reveal-bids.sh` on the machine with `target/market10-bids.txt` (running in this session) |
| 2 | The server keeper still runs the old `keeper.mjs` until the VPS pulls this commit | DEMO markets get every other slot (still at least 3 of 8 unless OutLayer fails twice) | Maris: `git pull` on the VPS |
| 3 | Aurora quote not configured | Pay page shows one quote of the two the description promises | Maris: `AURORA_INTENTS_API_KEY` in Vercel |
| 4 | Sponsor balance 8.47 NEAR | At 0.1 NEAR per guest about 75 guests; then the site is read-only (scripted demo still works) | Maris: top up `flatrate-sponsor.testnet` from the faucet or the root account |
| 5 | `tokens.flatrate-maris.testnet` still has a full-access key | The token market is not keyless (README says so) | Maris: "go keyless" if wanted |
| 6 | Token WEEKLY market 11 has no bids | It will open and settle with no claims | Optional: seed bids before its commit end |
| 7 | Proof page takes about a minute to fill on a cold load | A judge may see "…" for a while | Optional: cache or batch further |
| 8 | Shadeform aggregates other clouds, including Verda | Sources not fully independent (README Limitations) | — |
| 9 | Contract does not verify the TDX quote; OutLayer governance keys | Trust in OutLayer (README Limitations) | — |
| 10 | Description document still says "Nothing is built yet" and has rows marked "source link needed" | The description is out of date; README and CLAIMS are current | Maris |

## Submission checklist (CLAUDE.md)

| Item | Status |
|---|---|
| Repository public, default branch builds from a fresh clone | Yes (builds in WSL from a fresh clone of `6724695`) |
| `make verify` passes from a fresh clone | Yes on the VPS at `374f7d3`; in this audit all rows pass except the two network rows inside WSL, which pass from Windows |
| Hosted demo live until at least 25 November | Live now; depends on Vercel, the sponsor balance and the VPS keeper |
| README names Facedown | Yes |
| Demo video from `docs/demo-script.md` | Maris |
| X post (built at the NEAR Hackathon, @NEARProtocol, #NEARHackathon) | Maris |
| Submission form | Maris |
