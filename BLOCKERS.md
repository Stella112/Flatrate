# BLOCKERS

Open items, each with evidence. Updated 2026-11-12 (Phase 0).

## B1. Callback check in the spec does not match how OutLayer works (needs Maris's OK)

CLAUDE.md says `on_observation` "accepts a result only if its predecessor is the OutLayer contract and it matches a pending request id stored by `request_observation`".

Evidence (FACTS.md 3.3):
- The callback is chained with `.then` on the promise `request_execution` returns, so its predecessor is **our own contract** (`flatrate.<root>.testnet`), not `outlayer.testnet`. A check `predecessor == outlayer.testnet` would reject every real result.
- OutLayer does not return its request id to the caller (`contract/src/execution.rs:365`); it is only in the event log and in the module's env as `NEAR_REQUEST_ID`.

Proposed equivalent (same security, same error names):
- `on_observation(market_id, pending_id)` is `#[private]` in effect: if `predecessor != current_account_id` -> panic `ERR_NOT_OUTLAYER`. Only the contract's own promise chain from `request_execution` can reach it, so no outside account can post a price.
- `request_observation` stores an internal `pending_id` (counter) per market; the callback must find and remove it, else `ERR_UNKNOWN_REQUEST`. This stops replay.
- The oracle module reads `NEAR_REQUEST_ID` and returns it in its output; the contract stores it as `Observation.outlayer_request`, so the Proof page can show the real OutLayer request id.
- Attack button "Fake price" (direct call to `on_observation`) still fails with `ERR_NOT_OUTLAYER`.

## B2. OutLayer testnet smoke test not run (needs accounts from Maris)

Phase 0 acceptance needs one successful OutLayer testnet transaction hash. Not done because:
- No root testnet account named yet; `~/.near-credentials/testnet` does not exist on this machine.
- The HTTPS route also needs an OutLayer payment key (`X-Payment-Key`), created from a funded testnet account (`docs/CLI.md:270-275`).
- I did not create a testnet account myself; account creation is for Maris.
- A local build-and-run of the cloned `echo-example` with OutLayer's `wasi-test-runner` was blocked by this session's permission check (building third-party code). It is not required by Phase 0; it is needed in Phase 3 for the oracle and will need approval then.

Latency and cost per call will be measured in the smoke test. Known so far: estimated deposit 0.0070001 NEAR at default limits (FACTS.md 3.1).

## B3. GitHub repo `Stella112/flatrate` does not exist

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
