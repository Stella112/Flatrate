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
