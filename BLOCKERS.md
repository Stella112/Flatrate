# BLOCKERS

Open items, each with evidence. Updated 2026-11-12 (Phase 0).

## B1. Callback check in the spec does not match how OutLayer works: RESOLVED 2026-11-12

Decision (Maris: "choose the best that doesn't limit it"): option D below. Both gates (caller is the contract itself, id is pending) are required to record a price; the order of checks only decides which error an attacker sees, so all four attack buttons show distinct errors with no live request, no market period and no cost.

```
on_observation(market_id, pending_id, #[callback_result] result):
  1. pending_id < next_pending_id && !pending.contains(pending_id) -> panic ERR_UNKNOWN_REQUEST   // consumed id = replay
  2. predecessor != current_account_id                             -> panic ERR_NOT_OUTLAYER      // anyone but our own .then
  3. pending.remove(pending_id) is None                            -> panic ERR_UNKNOWN_REQUEST   // never issued (unreachable from self)
  4. Ok(Some(out)) -> validate (>=3 sources, 0 < price < 10*cap) and record; Ok(None) -> event; Err -> refund deposit to caller
```

Attack buttons: Fake price = made-up price with a never-issued id -> `ERR_NOT_OUTLAYER`; Replay = an id already used by a real observation -> `ERR_UNKNOWN_REQUEST`; Early settlement and Steal a note unchanged. Pending ids and used ids are public state, so step 1 leaks nothing. Tests: sandbox "non-OutLayer caller rejected" and "unknown request id rejected" (consumed id from an outside caller) both run against the deployed-style contract; no test-only code path.

Original analysis:

CLAUDE.md says `on_observation` "accepts a result only if its predecessor is the OutLayer contract and it matches a pending request id stored by `request_observation`".

Evidence (FACTS.md 3.3):
- The callback is chained with `.then` on the promise `request_execution` returns, so its predecessor is **our own contract** (`flatrate.<root>.testnet`), not `outlayer.testnet`. A check `predecessor == outlayer.testnet` would reject every real result.
- OutLayer does not return its request id to the caller (`contract/src/execution.rs:365`); it is only in the event log and in the module's env as `NEAR_REQUEST_ID`.

Proposed equivalent (same security, same error names):
- `on_observation(market_id, pending_id)` is `#[private]` in effect: if `predecessor != current_account_id` -> panic `ERR_NOT_OUTLAYER`. Only the contract's own promise chain from `request_execution` can reach it, so no outside account can post a price.
- `request_observation` stores an internal `pending_id` (counter) per market; the callback must find and remove it, else `ERR_UNKNOWN_REQUEST`. This stops replay.
- The oracle module reads `NEAR_REQUEST_ID` and returns it in its output; the contract stores it as `Observation.outlayer_request`, so the Proof page can show the real OutLayer request id.
- Attack button "Fake price" (direct call to `on_observation`) still fails with `ERR_NOT_OUTLAYER`.

## B2. OutLayer testnet smoke test not run (needs accounts from Maris): RESOLVED 2026-11-12

Maris created `flatrate-maris.testnet` and logged in the OutLayer CLI. Smoke test done through the real project: tx `4zoELfFtWriQPepF2jcc8q1abfWtHWpVjDjXQNn7ccmt`, 12 s, ~0.002 NEAR (FACTS.md 9.3). The local `wasi-test-runner` was built and used on the VPS.

Original note:

Phase 0 acceptance needs one successful OutLayer testnet transaction hash. Not done because:
- No root testnet account named yet; `~/.near-credentials/testnet` does not exist on this machine.
- The HTTPS route also needs an OutLayer payment key (`X-Payment-Key`), created from a funded testnet account (`docs/CLI.md:270-275`).
- I did not create a testnet account myself; account creation is for Maris.
- A local build-and-run of the cloned `echo-example` with OutLayer's `wasi-test-runner` was blocked by this session's permission check (building third-party code). It is not required by Phase 0; it is needed in Phase 3 for the oracle and will need approval then.

Latency and cost per call will be measured in the smoke test. Known so far: estimated deposit 0.0070001 NEAR at default limits (FACTS.md 3.1).

## B3. GitHub repo `Stella112/flatrate` does not exist: RESOLVED 2026-11-12 (created by Maris as `Stella112/Flatrate`, pushed)

`gh repo view Stella112/flatrate` -> "Could not resolve to a Repository". The local folder `C:\nearflatrate` was `git init`-ed with branch `main`; nothing pushed. Creating a public repo is for Maris (or Claude, on an explicit OK).

## B4. Not yet supplied (needed by later phases)

- Vercel or Netlify account and CLI (Phase 5). Neither CLI is installed.
- Sponsor testnet account for `api/sponsor` (Phase 5).
- Optional: Aurora Intents API key, 1Click JWT. Phase 0 step 8 skipped.

## B5. Risks to watch (not blocking)

- Price APIs worked without keys from this machine. Whether OutLayer workers can reach them is only known after the first real run (no domain allow-list is documented; outbound HTTP to public addresses is allowed, `WASI_TUTORIAL.md:521-524`).
- Shadeform answered without a key although this doc said it needs one; it may start requiring one.
- Shadeform aggregates other clouds, including Verda, so the five sources are not fully independent.
- The live docs (`llms-full.txt`) and the repo disagree on whether results are wrapped in `{success,result,error}`; the repo matches the deployed storage version. Confirm with the first real call.
- Suggestion: the void path refunds 0.5/0.5. Fine for the core; no change proposed.

## B6. Sandbox suite length vs the 10-minute `make verify` budget (Phase 2, not blocking yet)

The sandbox suite took 402 s on the 2-core VPS (the 200-bid test alone ~140 s). `make verify` must finish in under 10 minutes and will also build the oracle and read testnet. A cold build adds several minutes on a fresh machine. Option for Phase 6: run sandbox tests in parallel (separate sandboxes per test are already isolated), or let `make verify` reuse prebuilt wasm. To decide in Phase 6.

## B7. Reproducible oracle hash for `make verify`: RESOLVED 2026-11-12

The oracle is now built with OutLayer's pinned compiler image (`scripts/oracle-reproducible.sh`); two clean builds give `6c252d52...`, which is the version pinned for new markets (weekly market 5). Markets 0-4 keep `f55f2700...` (same source, plain cargo build); `make verify` checks that each pinned version is registered on OutLayer as the wasm with that hash. The reproducibility check needs Docker; without it the row says SKIP.

Original note:

`make verify` must check that the pinned version (`f55f2700...`) equals the hash of `oracle/` built from this commit. The current wasm was built with plain `cargo build` on the VPS (rustc 1.98.1). A judge's machine with another rustc may produce different bytes. Options: pin the toolchain with `rust-toolchain.toml` and rebuild/redeploy, or use OutLayer's reproducible Docker build (`scripts/build_github_wasm.sh`, docs/CLI.md) and redeploy that version. To decide in Phase 6; markets keep the version pinned at creation, so a new version needs new markets.

## B8. 1Click quote endpoint timing out (2026-11-12 ~20:20 UTC, not ours)

`POST https://1click.chaindefuser.com/v0/quote` with the exact Phase 0 request (`fixtures/1click-quote-request.json`, fresh deadline) now answers HTTP 500 `Failed to receive response within timeout of 25000ms for exchange "1Click-API:api-exchange" and routing key "quote"` (correlationId `58b40147-160f-4209-8e94-e208b7d80b23`). The same request returned a quote at 00:10 UTC. The Pay page shows 1Click's error and offers the recorded quote, labelled "recorded". Re-check before submission.

## B9. Flatrate description page could not be read

`https://claude.ai/artifact/FdtYGwJkuJcGs1PFtqycHq` (named in CLAUDE.md as the product description with the Demo section) answers "artifact not found or not shared with you" for this account. `docs/demo-script.md` follows the CLAUDE.md brief instead; Maris should check it against that page.

## B10. OutLayer testnet outage during the first v2 DEMO market (2026-11-13 ~11:44-11:50 UTC)

Every run for market 11 failed inside OutLayer: guest request `DEbDWb1SL2VsqKARNDVZ3K4CqzVYHHdWmGVQNfYds7N1` and keeper request `Do3i2x8hunaBS89X6WUjokzsxJAeFRyRjEvubFLsz21G` (OutLayer request 38198) both log `Execution promise failed: Failed. Refunded 5000000000000000000000 yoctoNEAR`; the contract recorded `observation_rejected: oracle_no_output` and freed the slot each time. A direct `outlayer run flatrate-maris.testnet/flatrate-oracle ...` with no contract involved failed the same minute (`the request failed with timeout error`), and `GET https://testnet-api.outlayer.ai/public/workers` showed the testnet workers with 35 s of uptime (restarting). The same oracle version recorded prices for market 5 at 00:16 and 06:16 UTC. Market 11 is expected to settle void; the settled DEMO run is repeated once OutLayer answers again.

Also found: the browser request waited for FINAL, which the RPC answers with HTTP 408 while the yield is open; the site now returns once the request is included and shows the result through the slot status.

## B6 update (2026-11-13): RESOLVED by decision

Following `docs/Flatrate-full-description-v2.md` ("The slower sandbox tests run under a separate test command") and Maris's choice on 2026-11-13, `make verify` no longer runs the sandbox suite; `make test` does. This differs from the build prompt's `make verify` step 2; the description was chosen where the two differed.

## B11: extra 1 (agent auto-hedging) cannot commit bids with a flatrate-only function-call key (2026-11-13)
- The build prompt says the agent holds "a function-call access key limited to the flatrate contract" and bids in auctions.
- In the deployed contract a bid is committed only through fUSD `ft_transfer_call` (`ft_on_transfer`, `contracts/flatrate/src/lib.rs:800`), which needs 1 yoctoNEAR attached and is a call to `fusd`, not `flatrate`.
- docs.near.org/protocol/access-keys: function-call keys "do not allow to attach NEAR tokens to the call" (FACTS.md section 13). So a flatrate-only key cannot commit.
- The contract is keyless, so a zero-deposit commit path cannot be added. The same holds for extras 2 and 3, which need contract changes (new index, enclave clearing).
- With a flatrate-only key the agent can do: reveal_bid, clear_auction, settle, redeem, withdraw_refund, claim_transfer, wrap_note, unwrap_note, redeem_note, roll_market, request_observation.
- Waiting for Maris to choose a design.
- Resolution (2026-11-13): Maris chose two keys, budget-capped. The agent commits with a full-access key on its own account and uses a flatrate-only function-call key for everything else; README Limitations says so. Run on DEMO market 13: PROGRESS.md "Extra 1".

## B12: extras 3, 4 and 5 not built (2026-11-13, after "build all extras")
- **Extra 3, enclave-cleared auction.** Needs new contract logic (accept a clearing result from an OutLayer component, with bids encrypted to it) plus a new OutLayer component and key handling. The H100 contract is keyless and cannot change; a third contract with this logic is a new design that cannot be built and tested before the 14 November deadline. Not started.
- **Extra 4, secondary trading on Rhea.** Rhea pools trade NEP-141 tokens. Flatrate claims are balances inside the contract, not NEP-141 tokens, and the keyless contract cannot add a token interface. A wrapper token would need a deposit notification that `claim_transfer` does not send. `ref-finance-101.testnet` is live (Phase 0), but there is nothing Flatrate can list on it. Not started.
- **Extra 5, mainnet pay-in and payout.** Spends real money; CLAUDE.md allows it only after Maris writes `approve mainnet payment`, and it needs a funded mainnet wallet. Not started.

## B13: cross-chain pay-in and payout reduced to a chain-signature demo (2026-11-13)
- The description's extra "cross-chain pay-in and payout on test networks" would move test dollars between Base Sepolia and NEAR. That needs a relayer that watches one chain and credits the other: a bridge we write ourselves (CLAUDE.md "Out of scope") with an operator (against "no operator"). Maris chose the reduced version.
- Built instead: `scripts/chainsig/sepolia.mjs`. A NEAR testnet account controls a Base Sepolia address through NEAR's MPC signer `v1.signer-prod.testnet` and signs a real Base Sepolia transfer from it. No fUSD or claim crosses chains.
- Funding the Base Sepolia address needs a faucet with a captcha or login, so Maris funds it once.

- Update 2026-11-14: extras 3 (enclave-cleared auction) and 4 (Rhea) are now built on a third deployment, `sealed.flatrate-maris.testnet`, with a `claim_transfer_call` path and a claim-token contract (PROGRESS.md "Private auction and Rhea"). Extra 5 (mainnet) is still not built.

## B14: OutLayer testnet failed every execution on 2026-11-14 (from about 09:15 UTC)
- Every `request_execution` returned "Execution promise failed: Failed", for the auction enclave (`pubkey` and `clear`, called directly and through the contract) and for the token-price oracle that worked the day before; `GET https://testnet-api.outlayer.ai/health` answered `ok`. Last success before it: H100 DEMO market 17, request 43790 at 09:02 UTC. Failed examples: requests 45203 (contract clear, [GrzKXuvM](https://testnet.nearblocks.io/txns/GrzKXuvMqk7NUvgR9UkgcKY6HnkT4kkn3sHtk6Y5pCCN)) and those in `scratchpad` watcher runs at 12:52, 12:58, 13:05 UTC.
- Effect: the private auction's enclave clearing could not run on testnet; sealed market 10 was abandoned and refunded. Price slots during the outage stay empty; a market without enough slots settles void at 0.50 / 0.50.
- The keeper now waits 10 minutes between clear attempts on a market (`CLEAR_RETRY_MIN`), so an outage does not drain its allowance.
