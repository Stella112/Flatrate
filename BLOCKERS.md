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

## B7. Reproducible oracle hash for `make verify` (Phase 6, not blocking yet)

`make verify` must check that the pinned version (`f55f2700...`) equals the hash of `oracle/` built from this commit. The current wasm was built with plain `cargo build` on the VPS (rustc 1.98.1). A judge's machine with another rustc may produce different bytes. Options: pin the toolchain with `rust-toolchain.toml` and rebuild/redeploy, or use OutLayer's reproducible Docker build (`scripts/build_github_wasm.sh`, docs/CLI.md) and redeploy that version. To decide in Phase 6; markets keep the version pinned at creation, so a new version needs new markets.
