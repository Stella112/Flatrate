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
