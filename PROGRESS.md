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

## Phase 4: Cover Note NFT (2026-11-12)

Contract: NEP-171 core (`nft_transfer`, `nft_transfer_call`, `nft_token`, `nft_resolve_transfer`), NEP-177 `nft_metadata` ("Flatrate Cover Note", `FLATNOTE`, `nft-1.0.0`) and NEP-181 enumeration, all delegated to `near-contract-standards` 5.30.1 `NonFungibleToken`. New: `wrap_note`, `unwrap_note`, `redeem_note`, view `note_info`. Token metadata title `Cover Note #<id>: <amount> <Side>, H100, period ending <YYYY-MM-DD>`; `extra` JSON holds market, side, amount, index, floor, cap, period end. The owner check runs first in `unwrap_note`/`redeem_note`, so a non-owner always gets `ERR_NOT_OWNER`. The contract pays mint storage (no attached deposit), like claims and bids.

Upgrade: `migrate` (`#[private]`, `#[init(ignore_state)]`) reads the Phase 2 state layout and adds the empty NFT collections. Tested in the sandbox against the exact wasm that was live on testnet (`contracts/flatrate/tests/res/flatrate_phase2_deployed.wasm`, sha256 `a7fbb3c7...`).

```
$ CARGO_BUILD_JOBS=2 make test      (VPS)
shared 18 passed; fusd 3 passed; flatrate unit 3 passed (incl. note_title_helpers); oracle 5 passed
sandbox: test note_unwrap_before_settle ... ok
         test note_wrap_transfer_new_owner_redeems_old_owner_cannot ... ok
         (+ 19 earlier tests ok)
         test result: ok. 21 passed; 0 failed; finished in 462.46s
exit=0
$ cargo test -p flatrate --test sandbox migrate
test migrate_from_phase2_state_keeps_markets_and_claims ... ok
```

Testnet redeploy: 1 NEAR top-up for the larger code ([E8PWJJmR](https://testnet.nearblocks.io/txns/E8PWJJmRFqSvv6f1mLy6wYWkhgicQzAKqJUfcdoiGPHc)); deploy `flatrate.wasm` sha256 `c8fd8da60950337088a89a04fd161bc9bfb5292b528b26bfce487f2fe9f74ab5` (408438 bytes) with init call `migrate` ([4znywm1S](https://testnet.nearblocks.io/txns/4znywm1SV5xvos54MJsupmEPY1FP6vT7oxuoxiA9Rkzj)). After: markets 0 (Void), 1 (Auction), 2 (Settled 493750, 3 observations) unchanged; oracle version still `f55f2700...`.

Testnet run, market 3 (19:50 to 20:06 UTC; no auction bids, so it opened with no opening price):
| Step | Tx | Result |
|---|---|---|
| create market 3 | [6sLU23Nd](https://testnet.nearblocks.io/txns/6sLU23NdzVHqfF41Xu4WSAxB9C9izAtkw1rNK4bFecWR) | |
| holder account + fUSD registration | [HvqGnBoB](https://testnet.nearblocks.io/txns/HvqGnBoBussULppWCEPeAiWwCFhtr48ALhCHvBvKSVFT), [6U1jix1z](https://testnet.nearblocks.io/txns/6U1jix1zhVXgASNGefZZkxKJG3b27344DZTHzxdcTjod) | `holder.flatrate-maris.testnet` |
| mint 10 pairs | [4WUUUpcu](https://testnet.nearblocks.io/txns/4WUUUpcuPeAhp6NQ4EP5L8g6td3MDQpPgTFYXjLFSJP4) | |
| **wrap** 10 Long | [3HZS2pZH](https://testnet.nearblocks.io/txns/3HZS2pZHbrUSvvQgzUxS1WrMStxF4RsJmCMgSAnnfGrA) | nft_mint token 0, "Cover Note #0: 10 Long, H100, period ending 2026-11-12" |
| **transfer** to holder | [3d2a5kcu](https://testnet.nearblocks.io/txns/3d2a5kcu5aPpB74XRKCszLvo2CtgBAVhmutaQbSVNGDJ) | nft_transfer, owner holder |
| observations | [Hpwa9NAL](https://testnet.nearblocks.io/txns/Hpwa9NALVhbS1ZXpaVsazbuYkWfBCngaiaDvsKKu7pUT), [DgCvofgo](https://testnet.nearblocks.io/txns/DgCvofgonnr9RtosoDej3JoMEozNhQ8rnewXm9ARHEY7), [2CJHtraG](https://testnet.nearblocks.io/txns/2CJHtraGCX1VnnCoqsbX4Ns2p45ZT8xJkZCbWVArbMy1) | 3980000 each, 5 sources, OutLayer requests 33678, 33695, 33710 |
| settle | [EC77TqDE](https://testnet.nearblocks.io/txns/EC77TqDEr7JXcMWu9zUzh382eqyWBPoqJQYF4Z864SJB) | long 493750 |
| old owner `redeem_note` | [EykARtbb](https://testnet.nearblocks.io/txns/EykARtbbnerbUyAwwTnDgzQNeqHCDHKzXiG1CaH3jg86) | fails `ERR_NOT_OWNER` |
| **redeem** by holder | [AV9VT9an](https://testnet.nearblocks.io/txns/AV9VT9anMcr4Kem2zQmCFfuoGPN6D4skptqgjfKZHduT) | 4937500 fUSD to holder (1000000 -> 5937500), nft_burn |
| root redeems its 10 Short | [4dHfzB4B](https://testnet.nearblocks.io/txns/4dHfzB4BY1oaVPTZvMgrzs8XhL3b4dZzjMSqWs8PVHwg) | 5062500; market collateral 0 |

### Acceptance
- Tests pass: PASS (sandbox 21/21 plus the migration test)
- Three testnet transaction hashes (wrap, transfer, redeem): PASS

## Phase 5: frontend, sponsor and hosting (2026-11-12)

- `web/`: Vite 8.3.5 + React 19.1.10 + TypeScript 7.0.2 + near-api-js 7.4.0 (versions pinned to releases at least two weeks old; `npm view <pkg> time`). Pages: Markets, Market (chart, sealed auction with local salt backup, mint/redeem/transfer, wrap, request observation, settle, redeem), My notes, Proof, Pay from any chain. Hash routing; no router or chart library.
- Guest accounts: the browser makes an ed25519 key (`localStorage`); the account id is its implicit id. `web/api/sponsor.js` sends 0.5 NEAR and, in one batch to fUSD, `faucet` + `storage_deposit` + `ft_transfer` of 100 fUSD. Rate limit 3 per IP per hour, in function memory only (best effort).
- The commitment hash is computed in the browser; checked equal to the contract's `bid_hash` view for 3 cases including a Unicode and an empty salt.
- Proof page: lists observation transactions from NearBlocks API v3 (no key, CORS `*`) and reads each receipt tree from `archival-rpc.testnet.fastnear.com`, matching by pending id; shows the OutLayer request id, per-source prices, OutLayer's charge, and a check that the stored price equals OutLayer's output.
- `web/api/quote.js`: 1Click `/v0/quote` with `dry: true`, USDC on EVM chains (asset ids from live `/v0/tokens`) to USDC on NEAR. 1Click was timing out during this phase (BLOCKERS B8); the page shows the error and offers the recorded quote, labelled "recorded".
- Scripted demo mode (`/?demo=scripted`): replays market 2 from `fixtures/demo/market-2.json` (recorded from public RPC by `scripts/record-demo-fixture.mjs`); badge on every screen; actions disabled.
- Hosting: Vercel project with Root Directory `web`, config `web/vercel.json`, env `SPONSOR_ACCOUNT_ID=flatrate-sponsor.testnet` and `SPONSOR_PRIVATE_KEY` set by Maris in Vercel. Two failed builds while fixing the layout (`npm ci --prefix`, then the Root Directory); functions moved to `web/api/`.
- **Public URL: https://flatrate-gpu.vercel.app/**

### Acceptance run on the hosted URL (2026-11-12, 20:46 to 21:14 UTC)

Fresh profile for that origin: the first visit created a new key, guest `25569026bb835b6129bbaa0d0962b12371a8d5bc06baa4eb5f718c3893f7eede` (not on chain before). Market 4 was created for the run ([Bp7jHQ7z](https://testnet.nearblocks.io/txns/Bp7jHQ7zAdXLm7QL1K2CNTikNGsFbC4x51ugyHBiPxwD)): commits 8 min, reveals 6 min, period 12 min, min 2 observations. Every step below was a button on the hosted site, signed by the guest key in the browser (hashes from NearBlocks for the guest account):

| Step | Tx | Result |
|---|---|---|
| guest account (sponsor) | [4JkHyZav](https://testnet.nearblocks.io/txns/4JkHyZavRtEkC9bm3aNFC8HevxUyRSr4Sv9yWgB4zvLi) | 0.5 NEAR, then 100 fUSD |
| faucet | [FtdQBw76](https://testnet.nearblocks.io/txns/FtdQBw765PiWg89dV3LtVqTAXkQmdmhjYWteLss1chUg) | +100 fUSD |
| commit Long 10 @ 55c (escrow 6) | [ETvoieX6](https://testnet.nearblocks.io/txns/ETvoieX6unbFhu4gWZDY5qBRcUH7vBhH2LFZq9UH4Swu) | sealed |
| commit Short 10 @ 50c (escrow 5) | [EuxgYc2H](https://testnet.nearblocks.io/txns/EuxgYc2H4rDx8vWemD3V5681BZwrSn2HcUDH1KW4veof) | sealed |
| reveal Long | [A8HdXSP3](https://testnet.nearblocks.io/txns/A8HdXSP3veJPQW6WdodqSrHo3rbPWKZbwBqNY5xXBsnS) | revealed |
| reveal Short | [95QnftYx](https://testnet.nearblocks.io/txns/95QnftYx173msSPkgnYrfHJLU1aXQPhR4aW4odafnc3x) | revealed |
| clear | [EV33D1WX](https://testnet.nearblocks.io/txns/EV33D1WXYGfp63JQuFv9fGqpEEFPY8fdUskwVjAbhTMG) | 50c, 10 pairs, 1 fUSD refund owed |
| request observation | [4Cqz4igh](https://testnet.nearblocks.io/txns/4Cqz4ighB3DrEEXg6N2hGmHQS9KpD9mNEcp4M8H2Mr72) | 3980000, 5 sources, OutLayer 34027 |
| mint 5 pairs | [FuEKCcDZ](https://testnet.nearblocks.io/txns/FuEKCcDZC4QmT8j3adj2Qw1LWnJfXaeVjdx3wjfoy9SY) | |
| wrap 5 Long | [GnxmBM8g](https://testnet.nearblocks.io/txns/GnxmBM8gw7jCqjBX1grDsxtAeB5Encw79oiF3uZ7C2B4) | Cover Note #1 |
| transfer note to holder.flatrate-maris.testnet | [ARf3TzuY](https://testnet.nearblocks.io/txns/ARf3TzuYyFJ8cPqFcTTGMDJQb932uUhih6xscoH5HHup) | owner holder (checked with `nft_token`) |
| withdraw refund | [GrZH5oPM](https://testnet.nearblocks.io/txns/GrZH5oPM3y1LSv19TM1wNSTRrHffpNp3QTiLA1JwDiAh) | 1 fUSD |
| request observation | [APAUp3G9](https://testnet.nearblocks.io/txns/APAUp3G9KmA8HbPyZdZ8bWy77Y3cgVzMDcM6QeCnnXAP) | 3980000, OutLayer 34041 |
| settle before period end | [2AeCmvMf](https://testnet.nearblocks.io/txns/2AeCmvMfBFUYwcx4a9gkQoSuG1A5Wv1SeQRdTdnehJVU) | fails `ERR_PERIOD_NOT_ENDED` |
| settle | [4wyFyQ4n](https://testnet.nearblocks.io/txns/4wyFyQ4n74AjKBbypsXyxy5i8GWX8kFwzshpQudi7BPc) | long 493750 |
| redeem (10 Long + 15 Short) | [8BkqBNEE](https://testnet.nearblocks.io/txns/8BkqBNEEbEA6YYMRYgb8t9PLbzmWKQYgziyhYnRHK6E) | 12.53125 fUSD (185 -> 197.53125) |
| holder redeems the note (CLI) | [GKH9gkZF](https://testnet.nearblocks.io/txns/GKH9gkZFraVpYDa9JW13dJd7LYtbE8xRtcbBvGQr1QQD) | 2.46875 fUSD; market 4 collateral 0 |

Screenshots: `docs/screens/01..09`. Found and fixed during the run: the auction panel vanished after clearing (it now stays and shows the clearing price); at 375 px the oracle hash overflowed (it now wraps).

### Acceptance
- Full flow from a clean browser profile on the hosted URL (guest, faucet, commit, reveal, clear, mint, wrap, transfer, settle, redeem): PASS
- Screenshots in docs/screens/: PASS

## Build prompt v2 (2026-11-13)

Maris sent a revised build prompt (time slots, market templates, public `roll_market`, Aurora quote, attestation on the Proof page, three tracks). Decisions in chat on 2026-11-13: fresh start in place (not a migration), OutLayer fees paid from a contract-held budget so the keeper runs on a function-call key, Maris adds `AURORA_INTENTS_API_KEY` in Vercel, Maris adds `docs/Flatrate-full-description-v2.md`.

### Phase 0 additions
- Official rules read from `https://nearhackathon.com/HACKATHON.md` (saved as `fixtures/HACKATHON.md`): one person may send at most 2 independent projects and each README names the other (Q-015), testnet-only projects are eligible (Q-010), the demo post says "built at NEAR HACKATHON" and tags @NEARProtocol and #NEARHackathon, the hosted demo stays up to 2026-11-18 and ideally 2026-11-25 (Q-012), add a LICENSE file (Q-014).
- Rhea: `near contract call-function as-read-only ref-finance-101.testnet get_number_of_pools` -> `2961`; the account holds contract code (sha256 `8918c505...`). Live on testnet; only relevant to the Rhea extra.
- Slot timing: OutLayer runs took 12-15 s from request to callback (FACTS.md 9.3, Phase 3 markets), under the 40 s limit, so the DEMO slot stays at 1 minute.
- Attestation: `GET https://testnet-api.outlayer.ai/attestations/by-request/{request_id}` (near-outlayer `API.md:124`, CORS `*`) returns `tdx_quote`, `worker_measurement`, `wasm_hash`, `executed_wasm_sha256`, `input_hash`, `output_hash`, `transaction_hash`. For request 33589: `input_hash` = sha256 of the market's oracle input and `output_hash` = sha256 of the exact `SuccessValue` bytes OutLayer returned to the contract (checked with Python).
- Aurora Intents quote: `POST https://intents-api.aurora.dev/api/quote/{apiKey}`, same request body as 1Click (docs.intents.aurora.dev, Swap API reference "Request a quote").

### Contract v2
- `contracts/flatrate`: templates `DEMO` and `WEEKLY` stored at init (no setter); `create_market(template, floor, cap)` owner-only, once per template; `roll_market(template)` public, only after the previous market is Settled or Void, band = settled average x0.6 / x1.4 rounded down to 10 cents (previous band after a void); one observation per slot (`ERR_SLOT_FILLED` when the slot has a price or a request in flight, `ERR_NOT_IN_PERIOD` outside the period), result written to the slot recorded at request time, slot freed on any rejection; settlement on the mean of filled slots, `settled_average_micros` stored; `request_observation` takes no deposit (the contract pays OutLayer from its balance above storage and a 0.5 NEAR reserve; `payer_account_id` = the contract, so OutLayer refunds it); `fund_observations` lets anyone add NEAR. `set_oracle_version` removed (version fixed at init). Storage under new byte prefixes; market ids from 10, note ids from 1000; `reset` (private, ignore_state) for the fresh start.
- `contracts/mock-outlayer`: `Hold` mode pauses with yield/resume; `mock_resume` answers later.
- Sandbox (VPS): `test result: ok. 29 passed; 0 failed` after two test fixes (budget parsed as u128; the held-request test now crosses one 10 s slot so it stays inside the 200-block yield window). New tests: `create_market_is_owner_only_and_once_per_template`, `second_request_in_filled_slot_fails`, `second_request_while_pending_fails_and_late_callback_lands_in_its_slot`, `failed_request_frees_the_slot`, `request_outside_period_fails`, `average_uses_filled_slots_only`, `roll_before_previous_done_fails_and_roll_after_settle_uses_the_average`, `roll_after_void_reuses_the_band_with_no_full_access_key`, `reset_over_deployed_state_starts_empty`. 200-bid clear: 36.35 TGas.
- Deployed: `flatrate.wasm` sha256 `1965f27612b387177c05c4003d6d7e9a0ee5fa577e549dedc82278276664e030` (430840 bytes) with init `reset`, tx [6TtmxvUv](https://testnet.nearblocks.io/txns/6TtmxvUvg5j9Gk691QUovZmPVEVbdH7m4iso33zPpYmD). `get_config`: version `6c252d52...`, observation deposit 0.006 NEAR, budget 0.6157 NEAR.

### Keeper on a function-call key
On the VPS: function-call key added to the keeper account `54cf6863...158e` for `clear_auction, request_observation, settle, roll_market` on `flatrate.flatrate-maris.testnet` (tx [CXF3hSog](https://testnet.nearblocks.io/txns/CXF3hSogFi7TxK8gpaneyT6wGjpebAKHNFtfq2Ejghfp)); full-access key deleted (tx [9Tuxo7dz](https://testnet.nearblocks.io/txns/9Tuxo7dzPjLCqLPaLjqtYi76U28J2pWnmvwN7sSi758T)). `near account list-keys` -> one key, `only do ["clear_auction", "request_observation", "settle", "roll_market"] function calls on flatrate.flatrate-maris.testnet`. Cron every minute with `flock`; rolls WEEKLY only.

### Markets
- WEEKLY market 10: `create_market` tx [4UzaURrs](https://testnet.nearblocks.io/txns/4UzaURrsN5UiRg66SFYfnLsxhyMdQWmirLckV8DCBJDE). Auction to 2026-11-14 13:23 UTC, then 28 six-hour slots, 14 needed.
- DEMO market 11, run on the hosted site by a new guest `cb4edd970f31d24a89611b3bd81dc91daae55dcba6c3eb6f182b0e8d344653a1` (browser key; the pane was hidden for part of the run, so some buttons were pressed through the page's own handlers with `element.click()` instead of a mouse click):

| Step | Tx | Result |
|---|---|---|
| guest funded by sponsor | [HBpv2MHw](https://testnet.nearblocks.io/txns/HBpv2MHwYGek7MmurXBuurSJVGSf8E8VNZ24op8DW3J4), [9Pnpp94Q](https://testnet.nearblocks.io/txns/9Pnpp94QfqueM9Ra8LjvVLC7XSk7B6QDmJBJPjDaPmu) | 0.5 NEAR, 100 fUSD |
| faucet | [CEhirJAo](https://testnet.nearblocks.io/txns/CEhirJAoqr4s2HvrJP2Q3Zgij93VC5jSfHFY65Y7wsNj) | +100 fUSD |
| create DEMO market 11 (owner, once) | [CUckbbRn](https://testnet.nearblocks.io/txns/CUckbbRnTs9xKZcGtr4t3erDamUoi5j8QSHoJpvwyWtc) | |
| commit Long 10 @ 55c / Short 10 @ 50c | [Ed942eQF](https://testnet.nearblocks.io/txns/Ed942eQFcoKrd8Avu8HT6nJDnSUE3EQ4NmbEfoupLza), [61o6jLrZ](https://testnet.nearblocks.io/txns/61o6jLrZwh856z2oth2N4uAvBc74dm8HxEnsknt2M1Jj) | sealed |
| reveal both | [7ea1xbdQ](https://testnet.nearblocks.io/txns/7ea1xbdQ4MhBDg1S3tmK1bjgrL5D99Lt1jHaQ8sPHpe6), [AA1JGiHM](https://testnet.nearblocks.io/txns/AA1JGiHM5G1yneQssGJXJ7qQYtmUk47KWa8kMqsgBf9n) | |
| clear | [Cm2g9Ve7](https://testnet.nearblocks.io/txns/Cm2g9Ve7s5S6czb1xnCY661ywbUMfKnmxk7qBgQE4sJ4) | 50c, 10 pairs |
| guest requests slot 1 | [DEbDWb1S](https://testnet.nearblocks.io/txns/DEbDWb1SL2VsqKARNDVZ3K4CqzVYHHdWmGVQNfYds7N1) | OutLayer run failed (outage, BLOCKERS B10): rejected, slot freed, deposit refunded |
| mint 5 pairs | [5roXfJ7z](https://testnet.nearblocks.io/txns/5roXfJ7zUyRdWpcnBgWKM5XVDbDkELAdyW3v1JfBEFUE) | |
| wrap 5 Long | [G2WH2YLY](https://testnet.nearblocks.io/txns/G2WH2YLYGQRgzBAGSR8nBGw2jADWDrjy5iAJYbViY1Wx) | Cover Note #1000 |
| withdraw refund | [AGqFSvDw](https://testnet.nearblocks.io/txns/AGqFSvDwB5Zqo7U2yHxto67TjnWuzCJVyqzRY82i7ToL) | 1 fUSD |
| transfer note to holder | [FSJ3gyc1](https://testnet.nearblocks.io/txns/FSJ3gyc15xd8osW8AMeCYQ5EzrFsA1hruHAXoVHzNXFK) | owner holder |
| keeper slots 3, 5, 6, 7 | [bKUPEvUe](https://testnet.nearblocks.io/txns/bKUPEvUeC9RZ96pKuL8F9t8xV2YUnSZp2dYh4W5FWas), [9nctEDcL](https://testnet.nearblocks.io/txns/9nctEDcL4zxqJZMcmsdan9qXUZJUX5qGtHTQtCDiB5gD), [ChxqAXaR](https://testnet.nearblocks.io/txns/ChxqAXaRMSMBGNzpW4PGjNZNoktzVSeL4Tv15BGoVGs9), [7e4pgnnn](https://testnet.nearblocks.io/txns/7e4pgnnnzQyHC85r2DXqy33tXpfqHcpDfqEqShVrtqfv) | 3980000 each, OutLayer 38206, 38218, 38226, 38237 |
| keeper settles | [H2HUitUD](https://testnet.nearblocks.io/txns/H2HUitUDJG2TUC8rvKWbx3HWF1fRVMv8Dex5fsoW5bLj) | average 3980000, Long 493750 |
| guest redeems 10 Long + 15 Short | [BGEz8gGT](https://testnet.nearblocks.io/txns/BGEz8gGTrpa578cgQDLTCyvXntdaJx9ZMR41GsiXM5ZH) | 12.53125 fUSD (185 -> 197.53125) |
| holder redeems the note | [2mrCGSf9](https://testnet.nearblocks.io/txns/2mrCGSf9E1ctpNR878N3NbcQbUaFRYAMFagCJ8qwixR7) | 2.46875 fUSD |

`node scripts/verify-chain.mjs --oracle-hash 6c252d52...`: market 11 payout recomputed PASS (4/8 slots, mean 3980000, long 493750); OutLayer attestation per recorded price PASS (4 match pinned wasm + market input); keyless PENDING.

Found and fixed during the run: the site waited for FINAL on `request_observation`; the RPC answers 408 while the yield is open, so the page stayed busy. It now returns once the transaction is included.

## Phase 6: keyless, public roll, attack buttons, fresh clone (2026-11-13)

### Keyless (after Maris wrote `go keyless`)
```
$ near account list-keys flatrate.flatrate-maris.testnet network-config testnet now
| 1 | ed25519:51Q6NWT52SMkTkvCmMjUX51M1MYkgHJZSKxsYgiuVF4z | 277369895000003 | full access |
$ near account delete-keys flatrate.flatrate-maris.testnet public-keys ed25519:51Q6NWT52SMkTkvCmMjUX51M1MYkgHJZSKxsYgiuVF4z network-config testnet sign-with-legacy-keychain send
Transaction ID: CYNnUzUJMmvrWXCBTaack7CHUc1sYQ76WxxBdBqqTX89
Access key <ed25519:51Q6NWT52SMkTkvCmMjUX51M1MYkgHJZSKxsYgiuVF4z> for account <flatrate.flatrate-maris.testnet> has been successfully deleted.
$ curl -X POST https://rpc.testnet.fastnear.com -d '{"method":"query","params":{"request_type":"view_access_key_list","finality":"final","account_id":"flatrate.flatrate-maris.testnet"}}'
{"result":{"block_hash":"4sc2E9v3LPY6TwABLRVoxsfuQQaAR83AhtVBzyUDN5uL","block_height":277508508,"keys":[]}}
```
(The first attempt stopped before sending: near-cli's RPC returned "rate limit" while fetching the nonce.)

### Market rolled by a non-owner after keyless
The guest `cb4edd970f31d24a89611b3bd81dc91daae55dcba6c3eb6f182b0e8d344653a1` pressed "Roll the next DEMO market" on settled market 11: `roll_market` tx [FHm7EtV2](https://testnet.nearblocks.io/txns/FHm7EtV2A5rB8j2HGT95bysHBy8pFRSFpTmNruhJxJne) -> market 12, `created_by` the guest, band floor `2300000`, cap `5500000` (market 11 average 3980000 x0.6 = 2.388 -> 2.30, x1.4 = 5.572 -> 5.50). The keeper (function-call key) cleared it ([1dPYvaYF](https://testnet.nearblocks.io/txns/1dPYvaYFCZQwEx2rv7SDx6aoLSjMF1UaYdxShqBWA9E)) and filled slots 0 and 2 ([48fWmqvG](https://testnet.nearblocks.io/txns/48fWmqvGoQcekXctbuX3hiko912HWur6a7zQgPbqc2rJ), [GhUFSnYc](https://testnet.nearblocks.io/txns/GhUFSnYcFiHdKCu3BRQpBEg3RjY2jmZ2zUg968i69cJa); OutLayer 38405, 38418).

### Attack buttons on the hosted Proof page (guest account, market 12, contract keyless)
Setup: the guest minted 5 pairs ([8z2sBvRe](https://testnet.nearblocks.io/txns/8z2sBvReuAhVwHdBv1DxVx83ZB5CcbEw9krj4PkA9JYU)), wrapped 5 Long as Cover Note #1001 ([HpVRTaV8](https://testnet.nearblocks.io/txns/HpVRTaV8R3QxRSwe7f1uixeNuWFn39JWUZ9waoJpnPp1)) and transferred it to `holder.flatrate-maris.testnet` ([84Ss8mqg](https://testnet.nearblocks.io/txns/84Ss8mqgsyXseeRTQ84ChA4iZnXdtu4xgST38DnVK2ZZ)).

| Button | Target | Tx (fails on-chain) | Error |
|---|---|---|---|
| Fake price | market 12, pending id 999999999 | [8btQUK27](https://testnet.nearblocks.io/txns/8btQUK27Vn2y5tBHbJLVoJLDJG7PRLAWfs29dkwcysiY) | `ERR_NOT_OUTLAYER` |
| Second price in the same slot | market 12, slot 3 (keeper's request in flight) | [Frg5Ei1c](https://testnet.nearblocks.io/txns/Frg5Ei1csteSamT86bEDPUELAgE7mcx9qzo5uZB2YvbU) | `ERR_SLOT_FILLED` |
| Early settlement | market 12 | [7qXXU3F7](https://testnet.nearblocks.io/txns/7qXXU3F7vjkkb85tTFzbDfSe92XgARm3JnsJHqxNxdJm) | `ERR_PERIOD_NOT_ENDED` |
| Steal a note | note #1001 owned by holder | [6oAz5cBe](https://testnet.nearblocks.io/txns/6oAz5cBeqGvZ4btNZqXvUFDtxP61HT5rR7CpKMwjno3a) | `ERR_NOT_OWNER` |

Screenshot: `docs/screens/11-attacks-v2-keyless.jpg`.

### Fresh clone, no NEAR credentials (VPS, HOME without ~/.near-credentials, commit c4af08c)
```
$ make vendor && make verify
FLATRATE VERIFY  (480 s, logs in target/verify/)
shared payoff/median/auction tests             PASS     test result: ok. 18 passed
contract unit tests                            PASS     7 passed
cargo near build (fusd, flatrate, mock)        PASS     flatrate.wasm fusd.wasm mock_outlayer.wasm
sandbox tests                                  INFO     run separately: make test
oracle parsing vs fixtures/prices              PASS     test result: ok. 5 passed
oracle reproducible build                      PASS     sha256 6c252d52804cb3f356ab1f5718069a57ab2d2fb96ad14e21eb18277fbcdaaf44
oracle wasm on fixtures (OutLayer runner)      PASS     median 3980000 = median(2.56, 3.976, 3.99, 3.98, 5.087)
contract readable                              PASS     flatrate.flatrate-maris.testnet: 2 markets ...
pinned oracle version = build of oracle/       PASS     contract 6c252d52804cb3f3… vs local reproducible build 6c252d52804cb3f3…
OutLayer version 6c252d5280… exists            PASS     markets [10,11] + new markets
market 11 payout recomputed                    PASS     4/8 slots, mean 3980000, long 493750 (recomputed 493750)
OutLayer attestation per recorded price        PASS     4 match pinned wasm + market input
contract has no full-access key                PASS     0 keys, none full-access
RESULT: no FAIL
verify_exit=0 verify_s=480
```
480 s on the 2-core VPS from a cold build (Docker image pull and two reproducible oracle builds included).

### Fresh clone `make test` (VPS, no NEAR credentials, commit c4af08c)
```
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 865.55s
test_exit=0 test_s=908
```

### WEEKLY market 10: two sealed bids (2026-11-13)
Two accounts committed opposite bids so the weekly market opens from a two-sided auction. Each escrow is 30 fUSD, more than either bid needs, so the deposit does not reveal the bid. Salts are in `target/market10-bids.txt` (gitignored) and are revealed by `scripts/reveal-bids.sh`.

| Account | Commit tx | Escrow |
|---|---|---|
| flatrate-maris.testnet | [A8RR7Njy](https://testnet.nearblocks.io/txns/A8RR7NjyjgLGusJZeTLdZ1F5hmAgZ2SvaBx156Nu2G5F) | 30 fUSD |
| holder.flatrate-maris.testnet | [5uN4cHfE](https://testnet.nearblocks.io/txns/5uN4cHfEUu3d4ZXvQQvCyWSnspkeddqn2ToQoM3u4og5) | 30 fUSD |

`get_market 10` after the commits: `status: Auction`, `bid_count: 2`. Reveal window from the contract: 2026-11-14 11:32:43 to 23:32:43 UTC; the keeper clears after that. The bid contents go here after the reveal.


## Extra 1: agent auto-hedging (2026-11-13, after `go extras`)

Design (Maris chose "two keys, budget-capped" after B11): the agent runs on its own account `agent.flatrate-maris.testnet` (created [8est56km](https://testnet.nearblocks.io/txns/8est56kmNDtBR8vyfTh4zXVUBf8m6V1QQRCgzqaqRqvV), 0.5 NEAR, 100 fUSD from the faucet [ebeo1SSe](https://testnet.nearblocks.io/txns/ebeo1SSemgkLa1GM3DNo891v9mwbSRbceENuK85Ts1V), fUSD storage [Axyar2EG](https://testnet.nearblocks.io/txns/Axyar2EGcVtUQgLHL6QjLvtsH7MWcEnNBMKmnNf9m3vG)). A full-access key signs only the commit (fUSD `ft_transfer_call` with 1 yoctoNEAR); a function-call key limited to `flatrate.flatrate-maris.testnet` (allowance 0.25 NEAR, add [319tMxqk](https://testnet.nearblocks.io/txns/319tMxqk8t7NLjaxtNR5Qz2xzrEuJBNhxNPCiWjSjzCv)) signs everything else. Both key files are outside the repository (`~/.near-credentials/testnet/agent.flatrate-maris.testnet/`), made by `scripts/agent/new-key.mjs`, which prints only the public key.

Code: `scripts/agent/plan.mjs` (pure sizing and pricing), `scripts/agent/plan.test.mjs`, `scripts/agent/hedge-agent.mjs` (runner, salts saved in gitignored `target/agent-state.json` before the commit is sent), `scripts/agent/forecast.example.json`. Makefile: `test-agent`, `agent-dry`, `agent`. `make verify` runs the planning tests.

```
$ node --test scripts/agent/plan.test.mjs
ℹ tests 5
ℹ pass 5
ℹ fail 0
```

A commit signed with the flatrate-only key is refused before sending:
```
Error:
   0: Error: Having a deposit with a function call action is not allowed with a function call access key.
```

DEMO market 13, run by the agent with `AGENT_TEMPLATES=DEMO AGENT_ROLL=DEMO`, `holder` as the other side:

| Step | Who / key | Tx | Result |
|---|---|---|---|
| roll_market DEMO | agent, function-call key | [2AGP2qVa](https://testnet.nearblocks.io/txns/2AGP2qVa6eWqyLRqcfPSg3SdBNULhThiXme5m4hW9mD6) | market 13, band 2.30 / 5.50 |
| commit | agent, full-access key | [6Y3CYqE8](https://testnet.nearblocks.io/txns/6Y3CYqE8eWvWozV52N3E5k1yAUAdfmmJnL1vpHWsEmF2) | Long 64 @ <= 56c (40 h x $3.20 x 0.5; fair 53c from market 12 average 3.98 + 3c), escrow 40 fUSD |
| commit | holder | [Ai2oimvw](https://testnet.nearblocks.io/txns/Ai2oimvwRY2hA3o5wbBqUZ2WGNH2ZgRM7eqoiF35FRXc) | Short 40 @ <= 46c, escrow 25 fUSD |
| reveal | agent, function-call key | [H1B8xAts](https://testnet.nearblocks.io/txns/H1B8xAtsJnFT6K7LHfXnPDXXKiQ3VUApKkF5Lkix1Thq) | |
| reveal | holder (`scripts/reveal-bids.sh`) | [Dfbcrk16](https://testnet.nearblocks.io/txns/Dfbcrk16Lkkpt8msSadXTA5xMuq3VEoHxTPbHjcKNReE) | |
| clear_auction | keeper | [ErLTs88x](https://testnet.nearblocks.io/txns/ErLTs88x196mHc5gycRJwJJpq6sCpdBWjiWj5A7DbaWo) | opened at 54c, 40 pairs (agent filled 40 of 64); the agent's own clear 2 s later was refused (already cleared) |
| withdraw_refund | agent, function-call key | [Dcib3gVP](https://testnet.nearblocks.io/txns/Dcib3gVPoYkpqM5DVcJMQ6wy16JH5Exvkw7tYwvTF3Tj) | 18.40 fUSD (40 - 40 x 0.54) |
| 6 slots | keeper / OutLayer 38600-38631 | see keeper log | 3.98 each; slot 6 `oracle_no_output` |
| settle | keeper | [4S5feCw7](https://testnet.nearblocks.io/txns/4S5feCw7VD9na2M2D716vKwbyNC5jkrjuy9DgvcyfnXE) | average 3.98, Long 0.525 |
| redeem | agent, function-call key | [5GP7jvPw](https://testnet.nearblocks.io/txns/5GP7jvPw5Tb5K3cBS4duiYAf5AtPyPD7EwUWhbeNuaGW) | 40 x 0.525 = 21.00 fUSD |

Agent fUSD: 100 -> 99.40 (paid 21.60 for the hedge, received 21.00; the price did not move, so the hedge cost its premium).


## Extra 6: wallet login (2026-11-13, "build all extras")
NEAR Wallet Selector 10.1.4 with Meteor, MyNearWallet and Intear (`web/src/wallet.ts`), loaded only when "Sign in with a NEAR wallet" is clicked. One signing path: `call()` in `web/src/near.ts` sends through the wallet when one is signed in, else through the guest key. Our own wallet list instead of `modal-ui` (it bundles React 18). Meteor's SDK needs Node's `Buffer`, `global` and `process` in the browser: provided before the modules load (`buffer` 6.0.3, Vite alias).

Local check: the list loads all three wallets; clicking MyNearWallet opens `testnet.mynearwallet.com` (the in-app browser blocks popups from automation, so the sign-in itself needs a person). No wallet-signed transaction yet.

## Extra 2: token-price market (2026-11-13)
- Contract: `index: Option<IndexId>` init field (default `H100_HOURLY`), new `TOKEN_PRICE` variant, Cover Note title from the index. The H100 contract is keyless and keeps running the `e2420a4` build. Unit test `index_names_and_labels`; sandbox `index_is_fixed_at_init_and_defaults_to_h100` (VPS: `3 passed` with `note_wrap_*` and `reset_*`).
- Oracle: new crate `oracle-tokens/` (so `oracle/` and its pinned hash stay unchanged: reproducible build still `6c252d52…`). `cargo test`: 3 passed. Reproducible build: `b7d4679204d59903d538196ff90e7f66f59962b680ee4961b23a43ace46548b3` (twice). Live OutLayer run [AV8piDb1](https://testnet.nearblocks.io/txns/AV8piDb1LvfK8SEUJHDYtdzitmdGND8JfS6NuK1wso6u): 7/7 providers, median 710000.
- Funding: two faucet accounts deleted into the root ([7PP14zn6](https://testnet.nearblocks.io/txns/7PP14zn6RYGi7aoATpT3gyfnmqataEghX27bbqdnq5wX), [2dqREGPJ](https://testnet.nearblocks.io/txns/2dqREGPJK7xTu3EgsAuSekcWMS2378CC5LWR3wkTrPQX)), root 0.41 -> 20.41 NEAR.
- Deploy (`scripts/deploy-tokens.sh`): account [H3EwLye5](https://testnet.nearblocks.io/txns/H3EwLye5g7RHUaNNrbqEf6m16r3usPou1BS2ZLY6hq8E), deploy + `new` [4WcQnvKX](https://testnet.nearblocks.io/txns/4WcQnvKXapf4KKLbEy2gk3zEyjjrGcUoRy57hv4ebrLr), fUSD storage [CLATSBr3](https://testnet.nearblocks.io/txns/CLATSBr3ErrNL5vnTbXa3HxKbF9mzAe6WGZPfVXBHYHa). Markets: DEMO 10 [7aSr1iRD](https://testnet.nearblocks.io/txns/7aSr1iRDkTPLMRynivnbJzBMd9YmMNqVHdJt6YnZhDU2), WEEKLY 11 [EmacMX2C](https://testnet.nearblocks.io/txns/EmacMX2CD9ViF4CKoY51RJsEDHSZm5WzqHRwc7G7LqmK), band 0.40 / 0.90 (0.71 x0.6 / x1.4, rounded down to 10 cents).
- Keeper: `keeper.flatrate-maris.testnet` ([DYanSTK8](https://testnet.nearblocks.io/txns/DYanSTK8q5yR2iMVjUoEbT2hvqMm2mgnxgX8K1eRbc5F)), function-call key for the token contract ([BggAbsmj](https://testnet.nearblocks.io/txns/BggAbsmjLaegRYuiNVPAJ91fbenBnRTtszEX8Gf9mZbt)), full-access key deleted ([9YRDSYZP](https://testnet.nearblocks.io/txns/9YRDSYZPUjkYsGHAxiGnmAKVt7xGNT8KUjAr5BPhZHkX)); VPS cron line with `FLATRATE_ID=tokens.flatrate-maris.testnet`.

Token DEMO market 10:

| Step | Tx | Result |
|---|---|---|
| commit root Long 20 @ <= 65c / holder Short 20 @ <= 40c | [866y9ZXb](https://testnet.nearblocks.io/txns/866y9ZXbAZGzpzuJZUFmnrfL9xbrGAkrTrFLsYzXFVk2), [fRRTkWKS](https://testnet.nearblocks.io/txns/fRRTkWKS1o5U88n3JxE9JY4QpLTFQWVdWr91gH8YVb8) | 15 fUSD escrow each |
| reveal | [AcDTAFZm](https://testnet.nearblocks.io/txns/AcDTAFZm5qA6BCJdhAEoppCh6aAVAEvjXBcti6emRLfg), [D1q2joF7](https://testnet.nearblocks.io/txns/D1q2joF7kyxBFrjGH3qRxo8cCniJvwCCjwGqyQFdRWKa) | |
| clear_auction | [CybVjMeu](https://testnet.nearblocks.io/txns/CybVjMeuhm4k4ChVnn79UEBX2EGzffpQfKzNftbAzfzQ) | opened at 60c, 20 pairs |
| 8 slots | OutLayer 40723-40757, e.g. [Lyv1jzXA](https://testnet.nearblocks.io/txns/Lyv1jzXAJeaMA3iPTUmMwx7oQYLjK87j3KiwjfdkZ5c), [6zjBC1vk](https://testnet.nearblocks.io/txns/6zjBC1vkuMWZ7RG3ov3bDgC958ycAzTf4M8e3eAvXQRh) | 710000 each (8/8) |
| settle | [2nVaJXL5](https://testnet.nearblocks.io/txns/2nVaJXL5AjvJf1CHpevVU6UzGVThD7Hs2KMYhcV9aKAw) | average 0.71, Long 0.62 ((0.71 - 0.40) / 0.50) |
| redeem | [DsN1Lo4J](https://testnet.nearblocks.io/txns/DsN1Lo4JDb4TWCoEzenByEWG1WYMKJq3WfQWP5YbEgVm), [J3oDL4GJ](https://testnet.nearblocks.io/txns/J3oDL4GJY4XX9nVcHP3FrfaypcXt44w78oyH2ghxbnKN) | 12.40 + 7.60 = 20.00 fUSD |

`node scripts/verify-chain.mjs` for the token contract: contract readable PASS, pinned version = build of oracle-tokens/ PASS, OutLayer version exists PASS, keyless PENDING (not asked).

## Extras 3, 4, 5
Not built; reasons in BLOCKERS.md B12.

### Final fresh clone after the extras (VPS, commit 374f7d3)
```
make verify   -> RESULT: no FAIL, 499 s (H100 and [tokens] rows; [tokens] keyless PENDING; OutLayer runner SKIP, not built in a fresh clone)
make test     -> test result: ok. 30 passed (sandbox), 914 s
```


## Extra: cross-chain, reduced to chain signatures (2026-11-13)
Maris chose the reduced version (BLOCKERS.md B13) and funded `0xF8677CE5ebc12A0E396e2747e4917C7C6E2dBe61` with 0.01 Base Sepolia ETH.
```
$ NEAR_ACCOUNT=holder.flatrate-maris.testnet node scripts/chainsig/sepolia.mjs send 0x701cbB65Bd7386222F76b27B4CF62666d950660E 1000000000000000
NEAR account holder.flatrate-maris.testnet, path "flatrate-payout" -> Base Sepolia 0xF8677CE5ebc12A0E396e2747e4917C7C6E2dBe61
balance 0.01 ETH
MPC sign request on NEAR: https://testnet.nearblocks.io/txns/6vjfR3iW1cFiqyfk4xaUPBmJXJfax5nthG67Z2oqttSA
Base Sepolia tx: https://sepolia.basescan.org/tx/0xa9924054d5f471a131709ac2e8d5f288f73d8446a9622256ac2ed0d496209898
status success, block 58426740, from 0xf8677ce5ebc12a0e396e2747e4917c7c6e2dbe61
```
The recipient is the root account's derived address for the same path.


## Audit (2026-11-14)
Full results in [audit.md](audit.md). Summary:
- Fresh guest on the hosted site ran DEMO market 14 end to end (roll, two sealed bids, reveal, keeper clear at 50c, refund, mint, wrap, note transfer, four attacks, keeper settle at average 3.8025 / Long 0.469531, redeem 15.30469 fUSD; the note's new holder redeemed 4.69531 fUSD). Guest NEAR spent: 0.0059.
- Fresh clone of `6724695` in WSL: `make test` -> `test result: ok. 30 passed; 0 failed` (983 s); `make demo-scripted` -> `SCRIPTED DEMO PASS`; `make verify` all rows PASS except the two chain-check rows (WSL's Node `fetch` times out); `node scripts/verify-chain.mjs` from Windows passes for both contracts.
- Fixed: Proof page transaction lookups (rate limited, `6724695`); keeper sends `request_observation` without waiting for OutLayer (tested on DEMO market 16); sponsor gives 0.1 NEAR instead of 0.5; guest button hides after funding; README names Autonomous Business.


## Private auction and Rhea (2026-11-14)
Code (commit `1183300`): `auction-enclave/` (OutLayer component: X25519 key derived from an OutLayer signing key, NaCl box opening, uniform-price clearing), contract paths `commit_sealed`, `request_enclave_key`, private `clear_auction`, `on_enclave_result`, `abandon_auction`, `claim_transfer_call`; `contracts/claim-token/`; `scripts/seal-bid.mjs`, `scripts/deploy-sealed.sh`, `scripts/rhea-pool.sh`; site page for sealed bids.

Tests (VPS, branch `private-auction`):
```
cargo test -p flatrate --lib                         -> ok. 5 passed
cargo test -p claim-token --lib                      -> ok. 3 passed
cd auction-enclave && cargo test                     -> ok. 5 passed
cargo test -p flatrate --test private_and_tokens     -> ok. 8 passed (125 s)
make test (whole suite)                              -> sandbox ok. 30 passed; private_and_tokens ok. 8 passed
```

Testnet:
- Keeper for the sealed contract: `sealkeeper.flatrate-maris.testnet` [F3JFnqdR](https://testnet.nearblocks.io/txns/F3JFnqdRebsjVnN3QZ5pgpu9oCS1hYUQA7mH1bfDAAY1), function-call key [GwVoF1nW](https://testnet.nearblocks.io/txns/GwVoF1nWaa1y4oespTmnqaFi9rnWsH2YNBXkEdnyGoZi), full-access key deleted [AHipmxbC](https://testnet.nearblocks.io/txns/AHipmxbCVwR3duNLFaHN1SipDptFgZPCKj9QKuFuzeXW); VPS cron line (no auto-roll).
- Sealed market 10 (two sealed bids from the previous session): nobody had called `clear_auction`. Clear sent [GrzKXuvM](https://testnet.nearblocks.io/txns/GrzKXuvMqk7NUvgR9UkgcKY6HnkT4kkn3sHtk6Y5pCCN) -> OutLayer request 45203 failed (B14), `enclave_rejected`. Direct `outlayer run` of the enclave (`clear`, then `pubkey`) and of the token oracle also failed. Market abandoned [5SgCMYvP](https://testnet.nearblocks.io/txns/5SgCMYvPeQ2RhzgL8YehJFVK7tRUpDYHbsCvZJ5iXtxL); refunds [7746fZkB](https://testnet.nearblocks.io/txns/7746fZkBrKoHHCpBaQAZx2KgdTM27ek38zokKiEh4Y7q) (12 fUSD), [FySiJ2EC](https://testnet.nearblocks.io/txns/FySiJ2EC4gemCxVTjH1neQkvaxNn6PnxZvRx5SYKMgYY) (11 fUSD).
- Sealed market 11: rolled [86M3mNZJ](https://testnet.nearblocks.io/txns/86M3mNZJnfF6sywYDqLMq9wnDjbN5KzS1Y21TVmfcBio); no bids, cleared without OutLayer [FZ2e8CUC](https://testnet.nearblocks.io/txns/FZ2e8CUCFZavx1wtddv7riaWfsWYD4nZKfpfUSvj6U1W); root minted 20 pairs [4K8Pnp7f](https://testnet.nearblocks.io/txns/4K8Pnp7fhPExvrM9aPForH9PkZDTXpPjVMe8ha55hBmS).
- Claim token `frl11.flatrate-maris.testnet` (FRL11): account [3P95t1FM](https://testnet.nearblocks.io/txns/3P95t1FMMKDxCkCKTP2yj7zdFAG6eBGdf2d6KJACwmb3), deploy + `new` [CNwQYBGn](https://testnet.nearblocks.io/txns/CNwQYBGn5mfXqnBd4j84uQRJGwM9S7SWFTUoFa6rrDpn); Rhea storage on it [CvtHdYaS](https://testnet.nearblocks.io/txns/CvtHdYaSTAZtX5Y6ZfdzYZgSejvPmr2FoB2ZDharBWb9); registered on Rhea [GKmyLtYZ](https://testnet.nearblocks.io/txns/GKmyLtYZnvPivoAah3i15KgGSAMZByPTDE8dME47Znkn); pool 2978 [JBToNmoA](https://testnet.nearblocks.io/txns/JBToNmoAfc1JHxSfJk5N8T2yyVGWWnapvFdkJz9VyKPY).
- `scripts/rhea-pool.sh` (LP root, trader holder, pool 2978): wrap 10 Long [CTpqr4f3](https://testnet.nearblocks.io/txns/CTpqr4f3eHGr8v35eNn3yYFd1r3UJAhDQwhk4JSAoHJA); deposit FRL11 [21zfLdkX](https://testnet.nearblocks.io/txns/21zfLdkXR3MTa7FY8edyvo3faRdWAbn7kDjkESn5LKLv) and fUSD [Ghw528nY](https://testnet.nearblocks.io/txns/Ghw528nYnWDVkabBSbq5kKJnepNVyGeNxwjg2DHQyXrB); add_liquidity 10 FRL11 + 5 fUSD [7knhP5Wi](https://testnet.nearblocks.io/txns/7knhP5Wi5PFLYitSuwGuYkkKUAuyKPRVbqvkRbsm8zHP); trader registers [A5sgz9vc](https://testnet.nearblocks.io/txns/A5sgz9vc4Q4csPr2GdpUN7tN2J8aNfymVAHsgn9poErM), [8vhhcg9U](https://testnet.nearblocks.io/txns/8vhhcg9Ufj3A7rZL9LMft8t3bgeSCHSwynAQFWpyDXzT), [7nUU8ksV](https://testnet.nearblocks.io/txns/7nUU8ksVcWnoPUj2PtA67AXvu1c1hWjmbbBfZdRLX3ev); deposits 1 fUSD [53U67fZu](https://testnet.nearblocks.io/txns/53U67fZutKPEp6uoNsR8JeqkwsfA25Zxkx7FLnZ5hEq8); swap -> 1663192 FRL11 [9ZnA1WBW](https://testnet.nearblocks.io/txns/9ZnA1WBWo5mpS85QeSpZ7Dm4Tp3P1YsVZvwQrjYEWU8C); withdraw [6PdWRK4p](https://testnet.nearblocks.io/txns/6PdWRK4pbAchdmcGzgnZyA5uJ6A6WKLRa8iccioK4CnW); unwrap into 1663192 Long claims of market 11 [Du8d96io](https://testnet.nearblocks.io/txns/Du8d96io8C5RXjXsrgdZgbRc2DUNcXtdPcCktBEmiJmS).
- Pool 2976 (FRL10, from the previous session) stays empty: market 10 never opened.

### Sealed market 12: the enclave clears live (2026-11-14, after OutLayer recovered at 13:38 UTC)
- New keepers with function-call keys and no allowance cap (`sealed-keeper.flatrate-maris.testnet`, `tokens-keeper.flatrate-maris.testnet`): the 0.5 NEAR allowance of the first keepers refused 300 Tgas calls (`NotEnoughAllowance`); the new accounts then also needed more balance (`NotEnoughBalance` at 0.5 NEAR; funded to 2.5 NEAR).
- Roll [4XJ5vnKL](https://testnet.nearblocks.io/txns/4XJ5vnKLmhJUL6EMGupMqQYwrqqAgShKvwuxkTsiyDVj); sealed bids with `scripts/seal-bid.mjs` (tweetnacl, same as the site): root Long 20 @ <= 55c [BDrwLKU4](https://testnet.nearblocks.io/txns/BDrwLKU4hxSsLmU6aBWY9qtK9qnnmFLyUwghQKsLF4Y3), holder Short 20 @ <= 50c [FJDWi98e](https://testnet.nearblocks.io/txns/FJDWi98epDJa2QLxmkApB5MLxQ2xEo2HfCdrBMxQimVG).
- Keeper `clear_auction` [AcNrMc8c](https://testnet.nearblocks.io/txns/AcNrMc8cDLunZBXUiAJrpfsem4AKE4Pxu2snFP7x98vW) -> OutLayer request 45595 -> `auction_cleared` price 50c, matched 20 pairs, valid 2, invalid 0.
- Prices: slot 1 [Ensm31Ht](https://testnet.nearblocks.io/txns/Ensm31HtEhXKQwACu6VGDoYTSquwkZ9L4X8rP9dJVrR4) recorded 3.98 (5 sources), slot 3 recorded; other slots missed (keeper balance), so 2 of 3 needed -> settled void [8MdUcx65](https://testnet.nearblocks.io/txns/8MdUcx65dr4CHeiPax4KaKFtu6QK3t5qyaeQNWTHN2YZ).
- Sealed market 13 (same bids): the clear at 14:15 UTC failed again at OutLayer (`enclave_no_output`); the keeper retries every 10 minutes.
