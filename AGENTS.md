# AGENTS.md

For AI reviewers. Plain steps; each says what to expect.

```
1. Run: make vendor && make verify   -> expect: a table, every row PASS (SKIP only where Docker or the OutLayer runner is missing; PENDING only for the keyless row until the owner approves), last line "RESULT: no FAIL", exit 0
2. Open: https://flatrate-gpu.vercel.app/#/proof -> expect: observations with OutLayer request ids, tx links, per-source prices, and a ✓ where the stored price equals OutLayer's output
3. Run: make demo-scripted           -> expect: full market lifecycle replayed from fixtures/demo/market-2.json, ending "SCRIPTED DEMO PASS: sandbox replay equals testnet market 2"
   Open: https://flatrate-gpu.vercel.app/?demo=scripted -> expect: the same market step by step, "Scripted demo" badge on every screen
4. Read: CLAIMS.md                   -> each row has a test name or a testnet transaction
5. Read: README.md "Limitations"
```

Requirements for steps 1 and 3: Linux or macOS (near-sandbox has no Windows build), Rust with `wasm32-unknown-unknown` and `wasm32-wasip2`, `cargo-near` 0.23, Node 20+. Docker is optional: with it, `make verify` rebuilds `oracle/` in OutLayer's pinned compiler image and compares the sha256 with the version pinned on-chain. `make oracle-runner` builds OutLayer's test runner (several minutes, once) so `make verify` can run the oracle wasm on the recorded price responses.

Where things are:

| Question | Look at |
|---|---|
| What was run, with real output | `PROGRESS.md` (one section per phase) |
| Where each external fact comes from | `FACTS.md` |
| What is unverified or failing | `BLOCKERS.md` |
| Payoff, median, auction clearing | `shared/src/lib.rs` |
| Contract | `contracts/flatrate/src/lib.rs` |
| Sandbox tests (incl. attack paths) | `contracts/flatrate/tests/sandbox.rs` |
| Oracle component | `oracle/src/lib.rs`, `oracle/src/main.rs` |
| Read-only testnet checks | `scripts/verify-chain.mjs` |
| Site and serverless functions | `web/src`, `web/api` |

Things to try on the hosted site, each a real testnet transaction from a guest account: the four buttons under "Try to break it" on the Proof page (expected errors `ERR_NOT_OUTLAYER`, `ERR_PERIOD_NOT_ENDED`, `ERR_UNKNOWN_REQUEST`, `ERR_NOT_OWNER`).
