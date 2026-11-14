#!/usr/bin/env node
// Private auction: seals one bid to the enclave key of a Flatrate contract and commits it with
// fUSD escrow. The bid's side, quantity and limit never appear on-chain; only the enclave can open
// them (auction-enclave/).
//
// Usage: node scripts/seal-bid.mjs <account> <market_id> <Long|Short> <qty> <limit_cents> <escrow_fusd>
// Env:   FLATRATE_ID (default sealed.flatrate-maris.testnet), FUSD_ID, NEAR_RPC, DRY=1 (print only)
// Needs: near (near-cli-rs) with the account's key in ~/.near-credentials (legacy keychain), and
//        web/node_modules (tweetnacl 1.0.3, the same library the site uses).
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(path.join(here, "../web/package.json"));
const nacl = require("tweetnacl");

const FLATRATE_ID = process.env.FLATRATE_ID || "sealed.flatrate-maris.testnet";
const FUSD_ID = process.env.FUSD_ID || "fusd.flatrate-maris.testnet";
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";

const [account, marketId, side, qty, limit, escrowFusd] = process.argv.slice(2);
if (!account || !marketId || !["Long", "Short"].includes(side) || !qty || !limit || !escrowFusd) {
  console.error("usage: seal-bid.mjs <account> <market_id> <Long|Short> <qty> <limit_cents> <escrow_fusd>");
  process.exit(2);
}

async function view(method, args) {
  const res = await fetch(RPC, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "query", params: { request_type: "call_function", finality: "final", account_id: FLATRATE_ID, method_name: method, args_base64: Buffer.from(JSON.stringify(args)).toString("base64") } }),
  });
  const j = await res.json();
  if (!j.result?.result) throw new Error(JSON.stringify(j.error || j.result));
  return JSON.parse(Buffer.from(j.result.result).toString());
}

/** NaCl box from a fresh ephemeral key: base64(ephemeral public key || nonce || box). */
export function seal(pubkeyHex, plain) {
  const pk = Uint8Array.from(Buffer.from(pubkeyHex, "hex"));
  const eph = nacl.box.keyPair();
  const nonce = nacl.randomBytes(nacl.box.nonceLength);
  const box = nacl.box(new TextEncoder().encode(JSON.stringify(plain)), nonce, pk, eph.secretKey);
  return Buffer.concat([Buffer.from(eph.publicKey), Buffer.from(nonce), Buffer.from(box)]).toString("base64");
}

const cfg = await view("get_config", {});
if (!cfg.enclave_pubkey) throw new Error(`${FLATRATE_ID} has no enclave key yet (request_enclave_key)`);
const plain = { market_id: String(marketId), bidder: account, side, qty: Number(qty), limit_cents: Number(limit) };
const ciphertext = seal(cfg.enclave_pubkey, plain);
const escrow = String(Math.round(Number(escrowFusd) * 1e6));
const msg = JSON.stringify({ action: "commit_sealed", market_id: String(marketId), ciphertext });
console.log(`sealed to ${cfg.enclave_pubkey.slice(0, 16)}…: ${ciphertext.length} base64 chars; escrow ${escrowFusd} fUSD`);
if (process.env.DRY) process.exit(0);
const r = spawnSync(
  "near",
  ["contract", "call-function", "as-transaction", FUSD_ID, "ft_transfer_call",
    "json-args", JSON.stringify({ receiver_id: FLATRATE_ID, amount: escrow, msg }),
    "prepaid-gas", "100 Tgas", "attached-deposit", "1 yoctoNEAR",
    "sign-as", account, "network-config", "testnet", "sign-with-legacy-keychain", "send"],
  { encoding: "utf8", input: "" },
);
const out = (r.stdout + r.stderr).replace(/\x1b\[[0-9;]*m/g, "");
console.log(out.match(/Transaction ID: \w+/)?.[0] || out.split("\n").slice(-4).join("\n"));
const ev = out.match(/EVENT_JSON:(\{[^\n]*bid_committed[^\n]*\})/)?.[1];
console.log(ev ? `bid_committed ${ev}` : out.match(/ERR_\w+/)?.[0] || "no bid_committed event (refunded?)");
