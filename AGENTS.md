# AGENTS.md

For AI reviewers. Plain steps; each says what to expect.

```
1. Run: make vendor && make verify   -> expect (a few minutes): a table; shared tests, contract builds, oracle checks and read-only testnet checks PASS (SKIP only where Docker or the OutLayer runner is missing; PENDING only for the keyless row until the owner approves); last line "RESULT: no FAIL", exit 0
2. Run: make test                    -> expect (several minutes, sandbox): "test result: ok. 29 passed" for the sandbox suite, plus shared, unit and oracle tests
3. Open: https://flatrate-gpu.vercel.app/#/proof -> expect: prices with OutLayer request ids, tx links, per-source prices and attestation checks
4. Run: make demo-scripted           -> expect: a recorded testnet market replayed in the sandbox, ending "SCRIPTED DEMO PASS: sandbox replay equals testnet market 2"
   Open: https://flatrate-gpu.vercel.app/?demo=scripted -> expect: the same market step by step, "Scripted demo" badge on every screen
5. Read: CLAIMS.md                   -> each row has a test name or a testnet transaction
6. Read: README.md "Limitations"
```

Requirements for steps 1, 2 and 4: Linux or macOS (near-sandbox has no Windows build), Rust with `wasm32-unknown-unknown` and `wasm32-wasip2`, `cargo-near` 0.23, Node 20+. Docker is optional: with it, `make verify` rebuilds `oracle/` in OutLayer's pinned compiler image and compares the sha256 with the version pinned on-chain. `make oracle-runner` builds OutLayer's test runner (several minutes, once) so `make verify` can run the oracle wasm on the recorded price responses.

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

Things to try on the hosted site, each a real testnet transaction from a guest account: the four buttons under "Try to break it" on the Proof page (expected errors `ERR_NOT_OUTLAYER`, `ERR_SLOT_FILLED`, `ERR_PERIOD_NOT_ENDED`, `ERR_NOT_OWNER`).
