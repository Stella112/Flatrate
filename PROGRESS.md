# PROGRESS

## Phase 0: verify everything (started 2026-11-12 ~00:00 UTC)

No product code. Outputs: `FACTS.md`, `BLOCKERS.md`, `fixtures/`.

### Step 1: tools
```
$ rustc --version        -> rustc 1.96.0 (ac68faa20 2026-05-25)
$ cargo --version        -> cargo 1.96.0 (30a34c682 2026-05-25)
$ rustup target add wasm32-wasip2   -> installed
$ irm https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.ps1 | iex
  Downloading cargo-near 0.23.0 (x86_64-pc-windows-msvc) ... everything's installed!
$ irm https://github.com/near/near-cli-rs/releases/latest/download/near-cli-rs-installer.ps1 | iex
  downloading near-cli-rs 0.31.1 (x86_64-pc-windows-msvc) ... everything's installed!
$ node --version -> v26.4.0 ; npm 11.17.0 ; pnpm 10.34.6
```
Vercel and Netlify CLIs not installed (Phase 5).

### Step 2: OutLayer docs
```
$ git clone --depth 50 https://github.com/fastnear/near-outlayer vendor/near-outlayer
  2d50bfab14389857e6e50e84e76d5652b944e1cc Wed Nov 11 14:11:03 2026 +0200
```
Example folders are empty submodules; cloned `out-layer/weather-example`, `echo-example` and `test-secrets-example` over HTTPS into `vendor/`. Facts in FACTS.md section 3.

### Step 3: RPC
```
$ curl -X POST https://rpc.testnet.fastnear.com -d '{"method":"status",...}'
  {"result":{"chain_id":"testnet",...,"protocol_version":88,...}}
$ view_account outlayer.testnet -> code_hash AsbxRWTQ2mQAazgz1bbAC72oZ1H8UKL33JKwZmG2pjn2
$ near contract call-function as-read-only outlayer.testnet get_storage_version ... -> "13"
$ estimate_execution_cost (1e9 instr, 128 MB, 60 s, 64 B) -> "7000100000000000000000"  (0.0070001 NEAR)
```

### Step 4: root account
Not done: no root account named; no `~/.near-credentials/testnet`. BLOCKERS B2.

### Step 5: OutLayer smoke test
Not done: needs the root account and an OutLayer payment key. BLOCKERS B2.

### Step 6: price sources
```
akash:     HTTP 200, 8861 bytes    -> h100 SXM5 price.med 2.56
vast:      HTTP 200, 11066 bytes   -> min dph_total/num_gpus over 4 H100 SXM offers 3.976
runpod:    HTTP 200, 5631 bytes    -> "NVIDIA H100 80GB HBM3" securePrice 3.99
shadeform: HTTP 200, 265249 bytes  -> median of 1x H100 sxm5 hourly_price [199,331,398,436,441] cents -> 3.98
verda:     HTTP 200, 162837 bytes  -> "1H100.80S.30V" price_per_hour "5.087"
sorted [2.56, 3.976, 3.98, 3.99, 5.087] median 3.98
```
All without a key. Raw responses in `fixtures/prices/`.

### Step 7: 1Click
```
$ curl -X POST https://1click.chaindefuser.com/v0/quote -d @fixtures/1click-quote-request.json
  HTTP 201 {"quote":{"amountIn":"10000000",...,"amountOut":"9994746",...,"timeEstimate":35,...}}
```
Saved to `fixtures/1click-quote.json`.

### Step 8: Aurora
Skipped: no key supplied.

### Step 9: bounds
floor 2.40 (`2_400_000`), cap 5.60 (`5_600_000`) from median 3.98.

### Acceptance
- FACTS.md has every item with a source: PASS for steps 1-3, 6, 7, 9; steps 4, 5 FAIL (blocked, B2).
- fixtures/ has raw responses: PASS.
- One successful OutLayer testnet tx hash: FAIL (blocked, B2).

### Decisions after the Phase 0 report
- B1 resolved (2026-11-12): `on_observation` is guarded by "caller is this contract" plus "pending id exists", with the replay check ordered first so attack buttons show distinct errors. The spec's "predecessor is the OutLayer contract" is not possible with OutLayer's promise-return flow (FACTS.md 3.3). Details in BLOCKERS.md B1.

## Phase 1: contracts core (2026-11-12)

Built and tested on a Linux VPS (Ubuntu 24.04.4, x86_64, rustc 1.98.1, cargo-near 0.23.0 from the official `cargo-near-v0.23.0` installer), because near-sandbox has no Windows build. The repo was copied there with `tar` (no `target/`, `vendor/`, `.git`).

```
$ CARGO_BUILD_JOBS=2 make test
-- test-shared (cargo test -p flatrate-shared)
test tests::payoff_at_floor_cap_mid ... ok
test tests::payoff_below_floor_and_above_cap ... ok
test tests::payoff_rejects_bad_bounds - should panic ... ok
test tests::payoff_rounds_long_down ... ok
test tests::void_payoff_is_half ... ok
test tests::prop_long_plus_short_is_one_dollar ... ok
test tests::prop_payoff_monotonic ... ok
test tests::prop_redeem_never_exceeds_collateral ... ok
test tests::prop_clearing_never_mints_more_than_escrow_covers ... ok
test tests::mean_and_median ... ok
test tests::two_bid_match ... ok
test tests::tie_break_prefers_50_then_lower ... ok
test tests::partial_fill_by_limit_then_commit_order ... ok
test tests::two_hundred_bids ... ok
(+ 4 more)
test result: ok. 18 passed; 0 failed
-- test-unit
fusd:     test result: ok. 3 passed; 0 failed
flatrate: test result: ok. 0 passed; 0 failed   (contract logic is covered by sandbox tests)
-- build (cargo near build non-reproducible-wasm)
target/wasm/flatrate.wasm       248724 bytes  sha256 4132a6e1ee150e9c8c3d93b740a91a904cbc076982c8c4c6350e00adc5cf42ac
target/wasm/fusd.wasm           149093 bytes  sha256 53ee615fd7088c459f049c037119d57aaff23ba43df1c96e7ccfd6a7971a7dba
target/wasm/mock_outlayer.wasm  117673 bytes  sha256 318763c26c8699729bca076a159ad8fca2a2033a8d2693b7d30b3e6fa641f01b
-- test-sandbox (near-workspaces)
test create_market_is_owner_only ... ok
test ledger_mint_transfer_redeem_pair ... ok
test mint_refused_while_in_auction_and_refunded ... ok
test observe_settle_redeem_and_double_redeem ... ok
test oracle_failed_run_and_refused_call_record_nothing ... ok
test oracle_fewer_than_3_sources_rejected ... ok
test oracle_non_outlayer_caller_rejected ... ok
test oracle_out_of_range_price_rejected ... ok
test oracle_unknown_request_id_rejected ... ok
test request_observation_outside_period_rejected ... ok
test settle_too_early_refused ... ok
test too_few_observations_waits_then_voids_then_second_settle_refused ... ok
test result: ok. 12 passed; 0 failed; finished in 154.13s
exit=0
```

### Acceptance
- `make test` passes: PASS
- `cargo near build` succeeds for fusd and flatrate (and mock-outlayer): PASS
- Test output pasted here: PASS

## Phase 2: sealed opening auction (2026-11-12)

Contract: `commit_bid` (fUSD `ft_transfer_call`, msg `{"action":"commit_bid","market_id":"<id>","hash":"<64 hex>"}`), `reveal_bid`, `clear_auction` (uses `flatrate_shared::clear_auction`), `withdraw_refund`, views `get_bids`, `refund_balance_of`, `bid_hash`. Commitment: `sha256(borsh(market_id: u64, side: Side, qty: u64, limit_cents: u8, salt: String))`, `qty` in whole claims. At most 200 bids per auction.

Changed from plan: `clear_auction` does not send refunds itself. 200 `ft_transfer` promises do not fit in one transaction's gas, so unused escrow (including unrevealed bids) is credited to the bidder's refund balance during clearing and paid by `withdraw_refund` (restored if the transfer fails).

Run on the VPS (same setup as Phase 1):
```
$ CARGO_BUILD_JOBS=2 make test
shared:   test result: ok. 18 passed; 0 failed
fusd:     test result: ok. 3 passed; 0 failed
flatrate: test tests::hex_round_trip_and_rejects_bad_input ... ok
          test tests::commitment_depends_on_every_field ... ok
          test result: ok. 2 passed; 0 failed
sandbox:  test auction_two_bid_match ... ok
          test auction_tie_break ... ok
          test auction_partial_fill ... ok
          test auction_unrevealed_bid_refunded ... ok
          test auction_wrong_salt_rejected ... ok
          test auction_late_reveal_rejected ... ok
          test auction_200_bids_clear_within_gas ... ok
          (+ the 12 Phase 1 tests, all ok)
          test result: ok. 19 passed; 0 failed; finished in 402.39s
exit=0
```

The first run printed the gas of the transaction-to-receipt step (0.31 TGas), not the function call. The test was fixed to sum the receipts executed by the flatrate account and to fail if it measures under 1 TGas, then re-run:
```
$ cargo test -p flatrate --test sandbox auction_200 -- --nocapture
CLEAR_200_GAS clear_auction receipt gas_burnt = 35773858897951 (35.77 TGas); transaction total = 36083348909401 (36.08 TGas)
CLEAR_200_RESULT price_cents=50 pairs=197000000 refunds alice=103080000 bob=103080000
test auction_200_bids_clear_within_gas ... ok
```
200 revealed bids (100 Long, 100 Short, limits 1-99), 197 pairs minted at 50c, fUSD conserved (pairs + refunds = total escrow). 35.77 TGas of the 300 TGas limit.

### Acceptance
- All auction tests pass: PASS
- Gas used by the 200-bid clear recorded: PASS (35.77 TGas)

## Phase 3: real prices through OutLayer (2026-11-12)

### Oracle component
- `oracle/` (Rust, `wasm32-wasip2`): fetches akash, vast, runpod, shadeform, verda (rules in FACTS.md section 4), median with `flatrate_shared::median` (the function the contract checks with), returns `request_id` from `NEAR_REQUEST_ID`. A `fixture_responses` input field replays recorded bodies with no HTTP (tests only; the contract never sends it).
- Native tests against `fixtures/prices/`: `cd oracle && cargo test` -> `test result: ok. 5 passed`.
- OutLayer local runner (`wasi-test`, near-outlayer 2d50bfa) on the VPS:
```
live:     Execution successful! Fuel consumed: 50658175 instructions
          {"index":"H100_HOURLY","price_micros":3980000,"sources":[akash 2560000, vast 2267222, runpod 3990000, shadeform 3980000, verda 5138000],...}
fixtures: Execution successful! Fuel consumed: 69902848 instructions
          {"index":"H100_HOURLY","price_micros":3980000,...,"fixture":true}
```
- Uploaded and deployed as project `flatrate-maris.testnet/flatrate-oracle`, version `f55f27004faa014bb08665f5cf55fab21418c7c2004d1e17fc8758580a8694fa` (FACTS.md 9.2). Smoke run on testnet: tx [4zoELfFt](https://testnet.nearblocks.io/txns/4zoELfFtWriQPepF2jcc8q1abfWtHWpVjDjXQNn7ccmt), 12 s, ~0.002 NEAR, 5/5 sources, OutLayer request 31496.

### Deploy
`fusd.flatrate-maris.testnet` and `flatrate.flatrate-maris.testnet` deployed, oracle version pinned in `get_config` and in every market (FACTS.md 9.1). Markets: 0 (demo, interrupted), 1 (weekly, 16-23 Nov), 2 (demo, completed).

### Keeper
`scripts/keeper.mjs`: for each Open market inside its period, reads OutLayer's `estimate_execution_cost` and calls `request_observation` with that deposit (+0.001 NEAR, refunded by OutLayer). No npm dependencies. `scripts/demo-market-testnet.sh` runs one market from reveal to redemption and logs every tx.

### Market 0 (interrupted)
The PC slept from 13:39 to 19:21 UTC, so the reveal window and the period passed with the script paused. What ran is still a real record of two paths: unrevealed bids cleared and refunded ([EBRPG4uR](https://testnet.nearblocks.io/txns/EBRPG4uRDbFk62MUHN7JrrnHi6oc1Dp4PjuvEY6BSabQ), revealed 0, matched 0), void settlement with 0 observations ([3ibECLWe](https://testnet.nearblocks.io/txns/3ibECLWeZ7G4JT7MJ9ZFkTUAV5qyL1Xio13Fz1V8BmRL)), full escrow back by `withdraw_refund` 11 fUSD ([A4QFJEnF](https://testnet.nearblocks.io/txns/A4QFJEnFNwrtBaMk3AzHMT2XP3RtBR9yFDzy8N19DR5f)). The script now retries sends after network errors; the keeper retries RPC reads.

### Market 2 (completed, 19:23 to 19:48 UTC)
| Step | Tx | Result |
|---|---|---|
| create (commit 4 min, reveal 4 min, period 16 min, min 3 obs) | [8pWiDZhu](https://testnet.nearblocks.io/txns/8pWiDZhuzr5YWBDUFCuHhQebVZJywF4aShDzay2pL3S3) | market 2 |
| commit Long 10 @ 55c, escrow 6 fUSD | [FtGfietX](https://testnet.nearblocks.io/txns/FtGfietXW3iEQEWd1nqt24YSrL3KcJ3h4unuaBzvaP5c) | hash only |
| commit Short 10 @ 50c, escrow 5 fUSD | [6EebToNW](https://testnet.nearblocks.io/txns/6EebToNWF8k886Ggd7WnDLcTeKMDYtBirxfqoRyJZwuW) | hash only |
| reveal Long | [CFBVocs3](https://testnet.nearblocks.io/txns/CFBVocs334kHjBYt5FCANjttTeYToxa5beZniovAmBUq) | bid_revealed |
| reveal Short | [C5RBz9Br](https://testnet.nearblocks.io/txns/C5RBz9Brjd1CQriS1WedrgrQW3MaQE4YNN8SiM7RpyUQ) | bid_revealed |
| clear_auction | [36ay9uLT](https://testnet.nearblocks.io/txns/36ay9uLTur5W25J1yuDngkVDE73Wf9xQTXKxb1vpJVry) | price 50c, 10 pairs |
| observation 1 | [J5eEofdJ](https://testnet.nearblocks.io/txns/J5eEofdJEPKpk1BMe5zy8Rj6rbV8mR5PhhoNUrVFAshk) | 3980000, 5 sources, OutLayer request 33589 |
| observation 2 | [3Wed41tV](https://testnet.nearblocks.io/txns/3Wed41tV6aTaJxWHpHUfjgsWYdTyGikrn6sxfsx7kSZd) | 3980000, 5 sources, request 33605 |
| observation 3 | [4zsSXYWo](https://testnet.nearblocks.io/txns/4zsSXYWoaxW37xZoiTj2QUv5kZV1khHt9s4AEr7494wb) | 3980000, 5 sources, request 33621 |
| settle | [DrfwCuCv](https://testnet.nearblocks.io/txns/DrfwCuCv7UV3mmkokj1pn8NHf7hHjYnj2UtJA878iZjF) | avg 3980000, long 493750, short 506250 |
| redeem (10 Long + 10 Short) | [457EHofV](https://testnet.nearblocks.io/txns/457EHofVsV1WBohX2a4GZnd9zN8VcYey3KbKX8wxTF4P) | 10 fUSD |
| withdraw_refund | [fhB4bGVE](https://testnet.nearblocks.io/txns/fhB4bGVEKgrq29YyvQ171oXErb7hV2ofzfF7kftehVg) | 1 fUSD (escrow 11 - paid 10) |

A fourth observation was skipped: the keeper's RPC read failed once ("fetch failed") at 19:43; three met the minimum. Long payout check: (3.98 - 2.40) / 3.20 = 0.49375 (same as `payoff_rounds_long_down`).

### Acceptance
- Testnet hashes for 3 observations, the settlement and a redemption in PROGRESS.md and CLAIMS.md: PASS
