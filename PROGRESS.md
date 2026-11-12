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
