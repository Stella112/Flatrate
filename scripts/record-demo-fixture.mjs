#!/usr/bin/env node
// Records one finished testnet market into fixtures/demo/market-<id>.json for scripted demo mode.
// Everything in the file comes from public RPC: the market view, its bids, and the receipts of
// the listed transactions (oracle output per observation). Nothing is invented.
//
// Usage: node scripts/record-demo-fixture.mjs <market_id> <steps.json>
// steps.json: [{"label": "...", "hash": "...", "signer": "..."}] in lifecycle order.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const RPC = "https://rpc.testnet.fastnear.com";
const ARCHIVAL = "https://archival-rpc.testnet.fastnear.com";
const [marketId, stepsFile] = process.argv.slice(2);
if (!marketId || !stepsFile) {
  console.error("usage: record-demo-fixture.mjs <market_id> <steps.json>");
  process.exit(2);
}

async function rpc(url, method, params) {
  for (let i = 1; ; i++) {
    try {
      const r = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "rec", method, params }),
      });
      const b = await r.json();
      if (b.error) throw new Error(JSON.stringify(b.error));
      return b.result;
    } catch (e) {
      if (i >= 4) throw e;
      await new Promise((r) => setTimeout(r, 2000 * i));
    }
  }
}

async function view(method, args) {
  const r = await rpc(RPC, "query", {
    request_type: "call_function",
    finality: "final",
    account_id: FLATRATE_ID,
    method_name: method,
    args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
  });
  return JSON.parse(Buffer.from(r.result).toString());
}

const decode = (b64) => Buffer.from(b64, "base64").toString();

const steps = JSON.parse(readFileSync(stepsFile, "utf8"));
const out = {
  recorded_at: new Date().toISOString(),
  source: `testnet ${FLATRATE_ID}, market ${marketId}; recorded from public RPC by scripts/record-demo-fixture.mjs`,
  config: await view("get_config", {}),
  market: await view("get_market", { market_id: String(marketId) }),
  bids: await view("get_bids", { market_id: String(marketId), limit: 200 }),
  steps: [],
};

for (const s of steps) {
  const tx = await rpc(ARCHIVAL, "EXPERIMENTAL_tx_status", { tx_hash: s.hash, sender_account_id: s.signer, wait_until: "FINAL" });
  const events = tx.receipts_outcome
    .flatMap((r) => r.outcome.logs)
    .filter((l) => l.startsWith("EVENT_JSON:"))
    .map((l) => JSON.parse(l.slice(11)))
    .filter((e) => e.standard === "flatrate" || e.standard === "nep171")
    .map((e) => ({ standard: e.standard, event: e.event, data: e.data[0] }));
  let oracle = null;
  for (const r of tx.receipts_outcome) {
    const v = r.outcome.status?.SuccessValue;
    if (r.outcome.executor_id === "outlayer.testnet" && v) {
      try {
        const j = JSON.parse(decode(v));
        if (j && j.sources) oracle = j;
      } catch {
        /* not the oracle output */
      }
    }
  }
  const charged = tx.receipts_outcome
    .flatMap((r) => r.outcome.logs)
    .map((l) => l.match(/yNEAR charged: "(\d+)"/)?.[1])
    .find(Boolean);
  out.steps.push({ ...s, block_timestamp: null, ok: !("Failure" in tx.status), events, oracle, outlayer_charged_yocto: charged || null });
  process.stderr.write(`recorded ${s.label} ${s.hash}\n`);
}

mkdirSync("fixtures/demo", { recursive: true });
const path = `fixtures/demo/market-${marketId}.json`;
writeFileSync(path, JSON.stringify(out, null, 2) + "\n");
console.log(`wrote ${path} (${out.steps.length} steps)`);
