# FACTS

Every external fact used in this build, with its source. Checked 2026-11-12 (UTC) unless noted.
Source forms: URL; `path:line` (relative to `vendor/near-outlayer` at commit `2d50bfab14389857e6e50e84e76d5652b944e1cc`, 2026-11-11, unless noted); or a command and its output.

## 1. Toolchain (Phase 0 step 1)

| Tool | Version | Source |
|---|---|---|
| rustc | 1.96.0 (ac68faa20 2026-05-25) | `rustc --version` |
| cargo | 1.96.0 (30a34c682 2026-05-25) | `cargo --version` |
| rustup | 1.29.0 | `rustup --version` |
| Rust targets | `wasm32-unknown-unknown`, `wasm32-wasip2` (added this session), `x86_64-pc-windows-msvc` | `rustup target list --installed` |
| cargo-near | 0.23.0 (installed this session from `https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.ps1`) | `cargo near --version` -> `cargo-near-near 0.23.0` |
| near (near-cli-rs) | 0.31.1 (installed this session from `https://github.com/near/near-cli-rs/releases/latest/download/near-cli-rs-installer.ps1`) | `near --version` -> `near-cli-rs 0.31.1` |
| node | v26.4.0 | `node --version` |
| npm / pnpm | 11.17.0 / 10.34.6 | `npm --version`, `pnpm --version` |
| git / gh | 2.53.0.windows.1 / 2.93.0 (logged in as `Stella112`) | `git --version`, `gh auth status` |
| Vercel CLI / Netlify CLI | not installed (needed in Phase 5) | `vercel --version` -> not found |

## 2. NEAR testnet RPC (step 3)

- `https://rpc.testnet.fastnear.com` responds: `status` -> `"chain_id":"testnet"`, `protocol_version":88`, latest block height ~277286806.
- `outlayer.testnet` exists: `view_account` -> `code_hash AsbxRWTQ2mQAazgz1bbAC72oZ1H8UKL33JKwZmG2pjn2`, balance ~82.5 NEAR.
- `ref-finance-101.testnet` exists with code (`code_hash AEAm8sQ5KYZRgnpq2CN2NhX6d8QV76iRtiDpy5Ai6zDX`). That it is Rhea's live testnet DEX is NOT verified (extra only; not needed for core).

Command used:
```
curl -s -X POST https://rpc.testnet.fastnear.com -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":"1","method":"query","params":{"request_type":"view_account","finality":"final","account_id":"outlayer.testnet"}}'
```

## 3. OutLayer (step 2)

Paths are relative to `vendor/near-outlayer` (commit `2d50bfab`). The example folders in that repo are empty git submodules; the examples were cloned separately over HTTPS into `vendor/weather-example` (`c059f7f`), `vendor/echo-example` (`79b0ad8`) and `vendor/test-secrets-example` (`42415d8`).

### 3.1 Live contract state (view calls, 2026-11-12)
- `outlayer.testnet` `get_storage_version` -> `"13"`, matching what this commit expects (`contract/README.md:254`). So the cloned source describes the deployed contract.
- `is_paused` -> `false`.
- `get_pricing` -> `["1000000000000000000000","100000000000000","100000000000000000","100000000000000000"]` = (base_fee 0.001 NEAR, per_million_instructions 1e14 yocto, per_ms 1e17 yocto, per_compile_ms 1e17 yocto) (`contract/src/views.rs:17-24`).
- `estimate_execution_cost({max_instructions:1e9, max_memory_mb:128, max_execution_seconds:60}, 64)` -> `7000100000000000000000` yocto = **0.0070001 NEAR**.
- Same with `{5e9, 128, 30}`, input 128 -> `4000500000000000000000` = **0.0040005 NEAR**.

### 3.2 `request_execution` (`contract/src/execution.rs:35-45`)
```rust
#[payable]
pub fn request_execution(&mut self, source: ExecutionSource, resource_limits: Option<ResourceLimits>,
    input_data: Option<String>, secrets_ref: Option<SecretsReference>, response_format: Option<ResponseFormat>,
    payer_account_id: Option<AccountId>, params: Option<RequestParams>)
```
- `source`: `{"Project":{"project_id":"<owner>/<name>","version_key":"<key>"|null}}` (`null` = active version, `execution.rs:992-1005`); or `{"WasmUrl":{"url","hash","build_target"}}` (https, hash = 64 lowercase hex) (`contract/src/lib.rs:150-170`, `contract/README.md:62-68`).
- `resource_limits`: `{max_instructions?: u64, max_memory_mb?: u32, max_execution_seconds?: u64}`, defaults 1e9 / 128 / 60 (`lib.rs:259-273`). **If omitted, the request is compile-only and nothing runs** (`execution.rs:79`). Hard caps 5e11 instr, 180 s, 4096 MB (`lib.rs:89-94`).
- `input_data`: string, given to the module on stdin (`contract/README.md:77-78`).
- `secrets_ref`: `{profile, account_id}` (`lib.rs:374-377`).
- `response_format`: `"Bytes" | "Text" | "Json"`, default Text (`lib.rs:241-254`).
- `payer_account_id`: receives refunds; default = predecessor (`execution.rs:280`).
- `params`: all fields `#[serde(default)]` (`lib.rs:192-236`).
- Deposit: must cover `estimate_execution_cost` + input storage, else panics "Insufficient payment" (`execution.rs:245-259`). Refund of `payment - cost` goes to `payer_account_id` (`execution.rs:652-657`).
- Gas: OutLayer reserves `MIN_RESPONSE_GAS = 50 TGas` from the caller's prepaid gas (`lib.rs:28`); the proxy tutorial tells users to attach 300 TGas (`wasi-examples/PROXY_CONTRACTS_TUTORIAL.md:350`).

### 3.3 How the result reaches our contract
- `request_execution` returns the yield promise (`env::promise_return`, `execution.rs:365`). The caller chains `.then(Self::ext(current).on_observation(...))`.
- The value our callback receives is the module's stdout JSON as-is when `response_format = "Json"`; `null` on a failed run or timeout. No OutLayer envelope (`execution.rs:893-916`; `PROXY_CONTRACTS_TUTORIAL.md:256-266`). Callback type: `#[callback_result] result: Result<Option<T>, PromiseError>`.
- On timeout/failure the callback sees `Ok(None)`; `Err(PromiseError)` only if `request_execution` panicked at the door, in which case the whole deposit returns to **our contract**, which must forward it (`execution.rs:703-792`; `PROXY_CONTRACTS_TUTORIAL.md:408-412`).
- **The callback's predecessor is our own contract, not `outlayer.testnet`** (standard NEAR `.then` semantics; `#[private]` in the tutorial, `PROXY_CONTRACTS_TUTORIAL.md:189`). See BLOCKERS B1.
- **The OutLayer request id is not returned to the caller** (only the promise, `execution.rs:365`). It appears in the `execution_requested` event log (`execution.rs:298-317`) and inside the module as env var `NEAR_REQUEST_ID` (`PROJECT.md:737`). See BLOCKERS B1.
- Reference proxy: tutorial CoinFlip (`PROXY_CONTRACTS_TUTORIAL.md:101-216`, callback 5 TGas static, OutLayer gets unused gas by weight 1); `wasi-examples/test-storage-ark/relay-contract/src/lib.rs:36-155` (near-sdk `=5.9.0`, request 100 TGas, callback 20 TGas).
- Yield window 200 blocks (`contract/README.md:137`).
- The live docs at `https://app.outlayer.ai/llms-full.txt` mention a `{success,result,error}` wrapper; the source above says there is none. Resolve with the first real call in Phase 3.

### 3.4 Projects and versions
- `create_project(name, source)` payable; id = `"{predecessor}/{name}"` (`contract/src/projects.rs:49-60`). `add_version`, `set_active_version`, `remove_version` (`contract/README.md:306-316`).
- **`version_key` = the wasm sha256 for WasmUrl sources** (`projects.rs:726-734`). Pinning = pass `version_key: "<sha256>"` in `source`. The project owner can still `remove_version` a pinned key (then requests fail "Version not found").
- Views: `get_project`, `get_version(project_id, version_key) -> {wasm_hash, source, added_at, is_active}`, `list_versions` (`projects.rs:616-697`).
- CLI (`docs/CLI.md`): `outlayer login testnet` (:67-74), `outlayer upload out/x.wasm` -> prints SHA256 and a `test.fastfs.io` URL (:352-360), `outlayer deploy <name> <wasm-url>` (:112-120, attaches 0.1 NEAR, refund of unused), `outlayer versions` (:372-374). Install: `cargo install outlayer-cli` or release binary (:16-30).

### 3.5 Secrets
- `outlayer secrets set '{"KEY":"..."}' --project owner/name [--profile p] [--access ...]` (`docs/CLI.md:180-212`). Module reads them with `std::env::var("KEY")` (`PROXY_CONTRACTS_TUTORIAL.md:326-331`).
- Access is judged against the **transaction signer**, not the proxy contract (`contract/README.md:617-640`).
- Not needed: all chosen price sources work without a key (section 4).

### 3.6 WASI component
- `vendor/weather-example/Cargo.toml`: `serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`, `wasi-http-client = "0.2"`, `urlencoding = "2.1"`; `[[bin]]`; release profile `opt-level="z"`, `lto=true`, `strip=true`. HTTP call: `wasi_http_client::Client::new().get(&url)...send()?` (`vendor/weather-example/src/main.rs:86-89`); input from stdin (:65-67); output via `print!` (:134).
- `outlayer` crate: "tested" version `0.2` (`wasi-examples/WASI_TUTORIAL.md:540-556`); in-repo SDK is 0.2.2 (`sdk/outlayer/Cargo.toml:3`). Not required for a plain HTTP fetcher; `NEAR_REQUEST_ID` can be read via `std::env::var`.
- Build: `cargo build --target wasm32-wasip2 --release`; must be a binary with `fn main()` (`WASI_TUTORIAL.md:295-305, 698-723`).
- Limits: per-HTTP-request timeout 30 s; outbound HTTP to public addresses only; stdout must be one JSON value for `Json` (`WASI_TUTORIAL.md:81-89, 320, 521-524`). No allow-list of domains is documented.
- Local runner: `wasi-examples/wasi-test-runner`, `cargo build --release`, then `wasi-test --wasm app.wasm --input '<json>'`; links wasi:http via `wasmtime-wasi-http = "28"` (`wasi-test-runner/Cargo.toml:14`, `src/main.rs:232`), but "does not apply the worker's outbound-HTTP rules" (`wasi-test-runner/README.md:27-28`).

### 3.7 HTTPS API
- `POST https://testnet-api.outlayer.ai/call/{owner}/{project}` needs header `X-Payment-Key: owner:nonce:secret`; no API-key alternative (`API.md:34-52`). Key: `outlayer keys create` (0.1 NEAR) then `outlayer keys topup <nonce> --usd N` funded with free testnet `usdc.fakes.testnet` (`docs/CLI.md:270-275`; `docs/TESTNET_USDC.md`). Sync response `{call_id, status, output, compute_cost, instructions, time_ms, attestation_url}` (`PROJECT.md:459-470`).

### 3.8 Trust caveat
- `docs/NON_CUSTODIAL.md:351-363`: governance currently runs on single keys OutLayer holds; the DAO account's full-access key can obtain the master secret outside an enclave; one account approves enclave builds; contracts are upgraded by key with no vote or delay. The operator can also refuse service (:345-347).

### 3.9 Not found in docs
- OutLayer testnet latency per call; to be measured (BLOCKERS B2).

## 4. GPU price sources (step 6)

All five candidates answered HTTP 200 **without an API key** on 2026-11-12 ~00:05 UTC. Raw responses: `fixtures/prices/<source>.json` (fetch time in `fixtures/prices/FETCHED_AT.txt`). Ornn was not tested: no public endpoint URL was found in this document or the sources read.

Proposed selection rule per source (H100 SXM 80GB, on-demand, USD per GPU-hour). These rules are what the oracle will implement in Phase 3:

| Source | Request | Selection rule | Value today | Key? |
|---|---|---|---|---|
| akash | `GET https://console-api.akash.network/v1/gpu-prices` | `models[]` where `model == "h100"` and `interface == "SXM5"` -> `price.med` (USD/hr) | 2.56 | no |
| vast | `GET https://console.vast.ai/api/v0/bundles/?q={"gpu_name":{"eq":"H100 SXM"},"rentable":{"eq":true},"type":"on-demand","limit":50}` | `offers[]` with `gpu_name == "H100 SXM"` and `rentable` -> min of `dph_total / num_gpus` | 3.976 (4 offers) | no |
| runpod | `POST https://api.runpod.io/graphql` body `{"query":"query { gpuTypes { id displayName memoryInGb securePrice communityPrice } }"}` | `data.gpuTypes[]` where `id == "NVIDIA H100 80GB HBM3"` -> `securePrice` | 3.99 | no (for this query) |
| shadeform | `GET https://api.shadeform.ai/v1/instances/types` | `instance_types[]` where `configuration.gpu_type == "H100"`, `configuration.interconnect == "sxm5"`, `configuration.num_gpus == 1` -> median of `hourly_price` (integer **cents**) / 100 | 3.98 (from 199, 331, 398, 436, 441) | no (this doc said it needs one; it answered without) |
| verda | `GET https://api.verda.com/v1/instance-types` (`https://api.datacrunch.io/v1/instance-types` returns byte-identical data) | element where `instance_type == "1H100.80S.30V"` -> `price_per_hour` (a **string**, e.g. `"5.087"`) | 5.087 | no |

Notes:
- Shadeform is an aggregator; its list includes a `verda` row (3.31), so Shadeform and Verda are not fully independent.
- Akash `price` also has `avg` 2.76 and `weightedAverage` 2.73; `med` chosen.
- Whether OutLayer workers may reach these hosts over HTTPS is checked in section 3.

Median of the five today: **3.98 USD/GPU-hr** (sorted: 2.56, 3.976, 3.98, 3.99, 5.087).

## 5. Proposed H100 market bounds (step 9)

- floor = 0.6 x 3.98 = 2.388 -> **2.40** -> `floor_micros = 2_400_000`
- cap = 1.4 x 3.98 = 5.572 -> **5.60** -> `cap_micros = 5_600_000`

## 6. NEAR Intents 1Click (step 7)

- OpenAPI spec: `https://1click.chaindefuser.com/docs/v0/openapi.yaml` (HTTP 200).
- `GET https://1click.chaindefuser.com/v0/tokens` -> 230 tokens; fields `assetId, decimals, blockchain, symbol, price, priceUpdatedAt, contractAddress, coingeckoId`.
  - USDC on Base: `nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near`, 6 decimals.
  - USDC on NEAR: `nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1`, 6 decimals.
- `QuoteRequest` required fields (openapi.yaml, schema `QuoteRequest`): `dry, swapType, slippageTolerance, originAsset, depositType, destinationAsset, amount, refundTo, refundType, recipient, recipientType, deadline`. Enums: `swapType` EXACT_INPUT | EXACT_OUTPUT | FLEX_INPUT | ANY_INPUT; `depositType`/`refundType` ORIGIN_CHAIN | INTENTS | CONFIDENTIAL_INTENTS; `recipientType` DESTINATION_CHAIN | INTENTS | CONFIDENTIAL_INTENTS.
- `POST /v0/quote` with `dry: true`, 10 USDC Base -> USDC NEAR: HTTP 201, `amountOut 9994746` (9.994746 USDC), `timeEstimate 35` s. Request: `fixtures/1click-quote-request.json`; response: `fixtures/1click-quote.json`. `refundTo` is the example address from the OpenAPI spec and `recipient` is `example.near`; nothing is sent with `dry: true`.
- First attempt with a malformed recipient (`flatrate.testnet-placeholder.near`) and refund address `0x…01` returned HTTP 400 `"Internal server error"`; the API validates addresses even on dry quotes.

## 7. Aurora Intents (step 8)

Skipped: no `AURORA_INTENTS_API_KEY` supplied.

## 8. Rust crates and sandbox (Phase 1)

| Fact | Value | Source |
|---|---|---|
| near-sdk | 5.30.1 (rust-version 1.95) | `cargo search near-sdk`; `near-sdk-5.30.1/Cargo.toml:14` |
| near-contract-standards | 5.30.1 | `cargo search near-contract-standards` |
| near-workspaces | 0.24.0 (rust-version 1.95) | `cargo search near-workspaces`; crate `Cargo.toml:14` |
| proptest | 1.12.0 | `cargo search proptest` |
| tokio | 1.54.1 | `cargo search tokio` |
| FT impl pattern | contract delegates `FungibleTokenCore`, `FungibleTokenResolver`, `StorageManagement` to a `FungibleToken` field; `FungibleToken::new(prefix)`, `internal_register_account`, `internal_deposit`, `FtMint{owner_id, amount, memo}.emit()` | `near-contract-standards-5.30.1/src/fungible_token/core.rs:1-75`, `core_impl.rs:40-121`, `events.rs:25-30` |
| `ft_on_transfer(sender_id, amount: U128, msg: String) -> PromiseOrValue<U128>` | returns unused amount | `near-contract-standards-5.30.1/src/fungible_token/receiver.rs:24-29` |
| Reading a promise result | `env::promise_result` is deprecated; use `env::promise_result_checked(idx, max_len) -> Result<Vec<u8>, PromiseError>`; `PromiseError` has `Failed`, `TooLong(usize)` and is non-exhaustive | `near-sdk-5.30.1/src/environment/env.rs:2264-2303`, `src/types/vm_types.rs:51-59`, compiler error E0004 |
| `store::LookupMap` key needs `Ord` | compiler error E0277 on a non-`Ord` key | `cargo near build` output |
| near-sandbox platforms | only linux-x86_64, linux-aarch64, darwin-arm64. **No Windows.** On this Windows machine sandbox tests run in WSL2 Debian 13.5 (Rust 1.99.1, cargo-near 0.23.0 installed there) | `near-sandbox-0.4.0/src/runner/mod.rs:87-104,163-164` |
| near-workspaces API used | `near_workspaces::sandbox().await`, `worker.root_account()`, `account.create_subaccount(name).initial_balance(..).transact()`, `account.deploy(&wasm)`, `account.call(id, fn).args_json().deposit().gas().transact()`, `contract.view(fn).args_json().await?.json()`, `worker.fast_forward(blocks)`, `worker.view_block().await?.timestamp()`, `ExecutionFinalResult::{is_success, failures, logs, total_gas_burnt}` | `near-workspaces-0.24.0/src/worker/mod.rs:52`, `worker/impls.rs:104,305,350`, `types/account.rs:77-176`, `types/block.rs:32`, `result.rs:99-320` |
| cargo-near output | `cargo near build non-reproducible-wasm --manifest-path <crate>/Cargo.toml --out-dir target/wasm` writes `target/wasm/<crate_name>.wasm` (`fusd.wasm`, `mock_outlayer.wasm`, `flatrate.wasm`) | `cargo near build ... --help`; build output |

## 9. Testnet deployment and OutLayer runs (Phase 3, 2026-11-12)

### 9.1 Accounts
| Account | Role | Evidence |
|---|---|---|
| `flatrate-maris.testnet` | root / owner, created by Maris via `near account create-account sponsor-by-faucet-service` | `near account view-account-summary flatrate-maris.testnet network-config testnet now` -> 10 NEAR, 1 full access key, no contract |
| `fusd.flatrate-maris.testnet` | fUSD (test stablecoin) | create tx `8Zr6bdYJyePE9fcVnp93PDRARWxEhUSRTQuZVddY3pb6`, deploy+init tx `6zxqztYZWe83dUi2HgFTrmtRcQhMDnQDQXuZk2t54uKD`; `ft_metadata` -> `symbol "fUSD"`, `decimals 6` |
| `flatrate.flatrate-maris.testnet` | Flatrate contract | create tx `Dbgxy4wf9DqLvALSgdJHLPFBT7k6fuqfJpZBz7mTgTwp`, deploy+init tx `FSJyTbqSBhCnTqzYum66gcLyrk9xtjPoum6DfpaKL6Mb`; fUSD storage registration tx `ChmWc5qEd33bbv29wqGqVssQbiEWhw8q77Fu8yBCPzGa` |

Deployed wasm sha256: `flatrate.wasm` `a7fbb3c7bd0d098d894f3575b875894ede32732b86330568e2c3495da14b0bb3` (305048 bytes), `fusd.wasm` `53ee615fd7088c459f049c037119d57aaff23ba43df1c96e7ccfd6a7971a7dba`, both built on the VPS by the Phase 2 `make test` run that passed.

`get_config` on `flatrate.flatrate-maris.testnet`: owner `flatrate-maris.testnet`, fusd `fusd.flatrate-maris.testnet`, outlayer `outlayer.testnet`, oracle project `flatrate-maris.testnet/flatrate-oracle`, version key `f55f27004faa014bb08665f5cf55fab21418c7c2004d1e17fc8758580a8694fa`, sources akash, vast, runpod, shadeform, verda, resource limits 1e9 instructions / 128 MB / 30 s.

### 9.2 Oracle project
- Built on the VPS: `cargo build --target wasm32-wasip2 --release` in `oracle/` -> `flatrate-oracle.wasm` 464577 bytes, sha256 `f55f27004faa014bb08665f5cf55fab21418c7c2004d1e17fc8758580a8694fa`. Crates: `wasi-http-client = "0.2"`, `serde`, `serde_json`, `flatrate-shared` (path). The `outlayer` SDK crate is not used.
- OutLayer CLI `outlayer-v0.2.3-x86_64-pc-windows-msvc.zip` from `https://github.com/out-layer/outlayer-cli/releases/tag/v0.2.3`. Login done by Maris (`outlayer login testnet`).
- `outlayer upload` -> tx `BbWZdK7zE2bxG1BCDTohDC6QwuUMJdSYicig6fkPzoCM`, URL `https://test.fastfs.io/flatrate-maris.testnet/outlayer.testnet/f55f27004faa014bb08665f5cf55fab21418c7c2004d1e17fc8758580a8694fa.wasm` (downloaded bytes hash to the same sha256).
- `outlayer deploy flatrate-oracle <url>` -> project `flatrate-maris.testnet/flatrate-oracle`. `get_version` -> `wasm_hash f55f2700...`, `is_active true`, source `WasmUrl` build_target `wasm32-wasip2`.

### 9.3 OutLayer testnet run (Phase 0 step 5 smoke test)
- `outlayer run flatrate-maris.testnet/flatrate-oracle '{"index":"H100_HOURLY","sources":[...5]}'` (on-chain): tx `4zoELfFtWriQPepF2jcc8q1abfWtHWpVjDjXQNn7ccmt`, CLI quote "~0.0020 NEAR", 12 s wall time from send to result. Output saved to `fixtures/oracle/outlayer-testnet-run.json`: median `3980000`, 5/5 sources, `request_id "31496"`.
- So: OutLayer workers can reach all five price APIs (B5 risk resolved); `NEAR_REQUEST_ID` is set; the result is the module's JSON with no `{success,result,error}` wrapper (B5 question resolved in favour of the repo source).
- Local runner (`wasi-test`, near-outlayer `2d50bfa`): live run 50,658,175 instructions, 2.0 s; fixture run (`--input-file`, all five `fixtures/prices/*.json`) 69,902,848 instructions, median `3980000`.
- Live prices today differ from the Phase 0 fixtures for vast (2.267 vs 3.976) and verda (5.138 vs 5.087); the median is unchanged.

### 9.4 Markets created
| id | Purpose | Times (UTC) | min obs | tx |
|---|---|---|---|---|
| 0 | demo | commit until +10 min, reveal +10 min, period 30 min, void 60 min after | 3 | `9FE5ZNdoNLe6kshLiSyrK8wVjNfnJKEFY9mzrCG9mdEk` |
| 1 | weekly | commit until 2026-11-14 12:00, reveal until 2026-11-15 12:00, period 2026-11-16 00:00 to 2026-11-23 00:00, void 2026-11-25 00:00 | 7 | `6bUXwvzCCJCLKWfTMKq3BWnHmp24DF2EuDDjFmPzgKvK` |
Both: floor `2_400_000`, cap `5_600_000` (section 5).

## 10. Frontend and hosting (Phase 5, 2026-11-12)

| Fact | Value | Source |
|---|---|---|
| npm versions (at least 2 weeks old) | near-api-js 7.4.0 (2026-10-23), vite 8.3.5 (2026-10-21), @vitejs/plugin-react 6.1.2, react and react-dom 19.1.10, typescript 7.0.2 | `npm view <pkg> time --json` |
| near-api-js 7.4.0 API used | `new JsonRpcProvider({url})`, `provider.callFunction({contractId, method, args})`, `provider.viewAccount({accountId})`, `new Account(id, provider, keyPairString)`, `account.signAndSendTransaction({receiverId, actions, throwOnFailure, waitUntil})`, `actions.functionCall(method, args, gas, deposit)`, `actions.transfer(amount)`, `KeyPair.fromRandom("ed25519")`, `KeyPair.fromString` | `web/node_modules/near-api-js/lib/index.d.ts`, `account-BfsHguAv.d.ts` |
| NearBlocks API | v1 and v2 retired on testnet; v3 `GET /v3/accounts/{account}/txns?limit=&next=` works without a key, results in `data`, cursor in `meta.next_page`, `actions[{action, method}]`; CORS `*` | `https://api-testnet.nearblocks.io/api-docs/migration`; curl |
| Archival RPC | `EXPERIMENTAL_tx_status` on `https://archival-rpc.testnet.fastnear.com` returns the full receipt tree, including OutLayer's result (`SuccessValue` = oracle JSON) and `[[yNEAR charged: "..."]]` logs; CORS `*` | curl |
| OutLayer charge per observation | 0.0012891, 0.0014748, 0.0012725 NEAR (market 2) | `fixtures/demo/market-2.json` |
| fUSD registration cost | `storage_balance_bounds` min = max = 1250000000000000000000 (0.00125 NEAR) | view call |
| 1Click auth | OpenAPI `securitySchemes`: bearer JWT | `https://1click.chaindefuser.com/docs/v0/openapi.yaml` |
| Hosted URL | https://flatrate-gpu.vercel.app/ (Vercel, Root Directory `web`) | Maris; curl HTTP 200 |

## 11. Proof layer (Phase 6, 2026-11-12)

| Fact | Value | Source |
|---|---|---|
| Reproducible oracle build | `bash scripts/oracle-reproducible.sh --twice` (OutLayer `scripts/build_github_wasm.sh`, image `outlayer/wasmedge-compiler@sha256:d9eb7bd9bab6c46b77f309ec33e94ca112883268a848dd1929289fade799cd27`, amd64, rustc 1.97.1) -> both builds `6c252d52804cb3f356ab1f5718069a57ab2d2fb96ad14e21eb18277fbcdaaf44` (455093 bytes) | VPS run, `~/oracle-repro.log` |
| Same wasm in the runner | live: 51,678,172 instructions, median 3980000; fixtures: 69,517,754 instructions, median 3980000 | `wasi-test` on the VPS |
| Version deployed | `outlayer upload` tx `oScKA25if4Zucp7aPmMurGoptnw4tEfhb9v9hhJMY46`; `add_version` tx `BZtMEPzr1c...` (NearBlocks list for flatrate-maris.testnet); `get_version` -> `wasm_hash 6c252d52...`, `is_active true` | CLI, view call |
| Pinned on the contract | `set_oracle_version` tx `4C9ipap9H8sJXuBjXZYd6mX75Dra38sNg3SmewjRLyiC`; `get_config.oracle_version_key = 6c252d52...` | view call |
| Weekly market 5 | create `FyySyexoYXA2Km48EmFr7bn3a28Tm7A26X7evfEHG8xH`; bids `HSNd9z5L...`, `3pNhQjRV...`; reveals `8tKfY6fV7r1vo6GyGWpkpnkwzRJqWKVQg84GsubN1uhW`, `BjTroSf5ZJAXh6anrqKriRnxkmy1evzdmuWfvSfmPRc4`; clear `Be51xoU5xjwvcp6mcswjxrivmfZaAwKifNCVXcxNAnHN` (50c); period 2026-11-12 21:49:06 to 2026-11-19 22:00 UTC, min 7 obs | near-cli output |
| Standing note for the "Steal a note" button | Cover Note #2 (5 Long, market 5): mint `9n53yEweMve5bykZyFmjwdhi2atSAXhAPZJtVBPnTTPJ`, wrap `HsbDndYYSUNPLEy4QV6weh7WXuq5qFg7UKSX7R3h2dpa`, transfer to `holder.flatrate-maris.testnet` `BvA9qWATb3cHPR3fDPSNFarJRnA4c9k8HVXxrDBJ4iuv` | near-cli output |
| Keeper | implicit account `54cf686374f338d49d9a8e12e9912b6fa7f10ee7d5fcc561b6faa4abfac6158e`, key generated on the VPS (`near account create-account fund-later use-auto-generation`), funded 1 NEAR tx `6YZtKcHM2VEiPeUhfeEzC9yzvXsaW6B9MM7bPXKvtw84`; cron `15 */6 * * *` runs `scripts/keeper-cron.sh` for market 5; first pass tx `CxdEPMiCdqvSrDyaxvsQH38ZvX6KEdYLYJRWkY6MpxWF` (OutLayer request 34321) | VPS crontab and log |
| near-cli `tokens send-near` to a new implicit account | waits for an interactive confirmation; `transaction construct-transaction ... add-action transfer` sends without asking | observed 2026-11-12 |

## 12. Sources for the product description (2026-11-13)

Checked by fetching each page; claims without a source were removed from `docs/Flatrate-full-description-v2.md`.

| Claim | Source | Supporting text |
|---|---|---|
| H100 $7-10/hr early 2024, $2-4 late 2025 | https://www.silicondata.com/blog/h100-rental-price-over-time | "Early rental prices routinely exceeded $7–$10 per GPU-hour"; "widely available for $2–$4 per hour" |
| H100 near $1.70 (July 2026); Kalshi forward curves (B200, H200, A100) | https://aiweekly.co/alerts/kalshi-launches-gpu-compute-forward-curves-for-ai-hedging | "H100s currently trade near $1.70" |
| RunPod $2.69 vs Shadeform $4.17 on 13 Oct | https://github.com/Cassxbt/piry (README line 54) | "On 13 Oct the same H100 SXM hour was $2.69 on RunPod ... and $4.17 across Shadeform's clouds" |
| CME compute futures under regulatory review | https://www.fia.org/marketvoice/articles/exchanges-eye-turning-ai-compute-tradable-commodity (4 Nov 2026) | "The contracts are under regulatory review." |
| One CME contract = 730 GPU-hours | https://www.fia.org/fia/articles/explainer-what-are-compute-futures-and-how-do-they-work | "Each contract represents 730 GPU-hours" |
| Bernstein: "dominated by speculative flows"; Architect AX perpetual-style compute futures | https://theblock.co/post/408758/crypto-style-derivatives-reach-ai-compute-ahead-of-planned-cme-and-ice-futures-bernstein | quote as given |
| CME + Silicon Data compute futures | https://www.cmegroup.com/media-room/press-releases/2026/8/11/cme_group_and_silicondatatolaunchcomputefuturesonoctober5tounloc.html | press release (page timed out on fetch; details also in the FIA explainer) |
| ICE + Ornn compute futures | https://finance.yahoo.com/markets/options/articles/ice-ornn-launch-gpu-compute-142609556.html | "based on Ornn's Compute Price Index (OCPI)" |
| SealedSwarm, C0-op, Flock (this hackathon) | https://nearhunt.xyz/p/dbedb794-5294-4c07-a914-074eead64668, https://nearhunt.xyz/p/3c93bf03-e823-4fab-b708-9d067a790162, https://nearhunt.xyz/p/fa6703a5-2e4c-4721-9bdb-8f22915922f5 | project descriptions on NEAR Hunt |
| Published attacks on Intel TDX | https://research.uni-luebeck.de/en/publications/tdxdown-single-stepping-and-instruction-counting-attacks-against-/ (TDXdown, ACM CCS 2024, CVE-2024-27457); https://eprint.iacr.org/2025/079 | "single-stepping and StumbleStepping, work on the most recent Intel TDX enabled Xeon Scalable CPUs" |
| Removed (no source found) | Ornn index $3.30 on 9 Nov 2026, +25% in a month | nearest found: +22% to $3.28 in September 2026 (KuCoin flash), not the stated date |
