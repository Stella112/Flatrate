#!/usr/bin/env node
// Flatrate keeper: asks for a price observation on every market that is Open and inside its
// period. Anyone can run it; the caller pays the OutLayer deposit and gets any unused part back
// (OutLayer refunds `payer_account_id`, which the contract sets to the caller).
//
// No npm dependencies: view calls go to public JSON-RPC with fetch; the transaction is signed
// by near-cli-rs with the caller's key from ~/.near-credentials (legacy keychain).
//
// Usage:
//   KEEPER_ACCOUNT=<your>.testnet node scripts/keeper.mjs            # loop every 8 minutes
//   KEEPER_ACCOUNT=<your>.testnet node scripts/keeper.mjs --once     # one pass, then exit
// Env (defaults): FLATRATE_ID=flatrate.flatrate-maris.testnet  OUTLAYER_ID=outlayer.testnet
//                 NEAR_RPC=https://rpc.testnet.fastnear.com  KEEPER_INTERVAL_MIN=8
//                 KEEPER_MARKETS=<comma-separated ids, default: all>

import { spawnSync } from "node:child_process";

const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const OUTLAYER_ID = process.env.OUTLAYER_ID || "outlayer.testnet";
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const ACCOUNT = process.env.KEEPER_ACCOUNT;
const INTERVAL_MIN = Number(process.env.KEEPER_INTERVAL_MIN || 8);
const ONLY = (process.env.KEEPER_MARKETS || "").split(",").filter(Boolean);
const ONCE = process.argv.includes("--once");
// Extra yoctoNEAR on top of OutLayer's estimate; refunded by OutLayer if unused.
const MARGIN_YOCTO = 1_000_000_000_000_000_000_000n; // 0.001 NEAR

if (!ACCOUNT) {
  console.error("Set KEEPER_ACCOUNT to the testnet account that signs and pays.");
  process.exit(2);
}

// Retries transient network errors (public RPCs drop requests now and then).
async function view(contract, method, args) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await viewOnce(contract, method, args);
    } catch (e) {
      if (attempt >= 4) throw e;
      log(`view ${method} failed (${e.message}), retry ${attempt}`);
      await new Promise((r) => setTimeout(r, 3_000 * attempt));
    }
  }
}

async function viewOnce(contract, method, args) {
  const res = await fetch(RPC, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: "keeper",
      method: "query",
      params: {
        request_type: "call_function",
        finality: "final",
        account_id: contract,
        method_name: method,
        args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
      },
    }),
  });
  const body = await res.json();
  if (body.error || body.result?.error) throw new Error(JSON.stringify(body.error || body.result.error));
  return JSON.parse(Buffer.from(body.result.result).toString());
}

function log(...a) {
  console.log(new Date().toISOString(), ...a);
}

function requestObservation(marketId, depositYocto) {
  const near = (Number(depositYocto) / 1e24).toFixed(6);
  const args = JSON.stringify({ market_id: String(marketId) });
  const r = spawnSync(
    "near",
    [
      "contract", "call-function", "as-transaction", FLATRATE_ID, "request_observation",
      "json-args", args, "prepaid-gas", "300 Tgas", "attached-deposit", `${near} NEAR`,
      "sign-as", ACCOUNT, "network-config", "testnet", "sign-with-legacy-keychain", "send",
    ],
    { encoding: "utf8" },
  );
  const out = (r.stdout + r.stderr).replace(/\x1b\[[0-9;]*m/g, "");
  const tx = out.match(/Transaction ID: (\w+)/)?.[1];
  const events = [...out.matchAll(/EVENT_JSON:(\{.*\})/g)].map((m) => JSON.parse(m[1]));
  const ev = events.find((e) => e.event === "observation_recorded" || e.event === "observation_rejected");
  return { ok: r.status === 0, tx, event: ev, tail: out.split("\n").slice(-6).join("\n") };
}

async function pass() {
  const cfg = await view(FLATRATE_ID, "get_config", {});
  const markets = await view(FLATRATE_ID, "list_markets", { from_index: "0", limit: 100 });
  const nowNs = BigInt(Date.now()) * 1_000_000n;
  for (const m of markets) {
    if (ONLY.length && !ONLY.includes(m.id)) continue;
    const inPeriod = nowNs >= BigInt(m.period_start) && nowNs < BigInt(m.period_end);
    if (m.status !== "Open" || !inPeriod) continue;
    const limits = {
      max_instructions: Number(cfg.resource_limits.max_instructions),
      max_memory_mb: cfg.resource_limits.max_memory_mb,
      max_execution_seconds: Number(cfg.resource_limits.max_execution_seconds),
    };
    const estimate = BigInt(
      await view(OUTLAYER_ID, "estimate_execution_cost", { resource_limits: limits, input_len: m.oracle_input.length }),
    );
    const deposit = estimate + MARGIN_YOCTO;
    log(`market ${m.id}: requesting observation, deposit ${deposit} yocto`);
    const r = requestObservation(m.id, deposit);
    if (!r.ok) {
      log(`market ${m.id}: transaction failed\n${r.tail}`);
      continue;
    }
    log(`market ${m.id}: tx ${r.tx} ${r.event ? JSON.stringify(r.event.event) + " " + JSON.stringify(r.event.data[0]) : "(no event in output)"}`);
  }
}

for (;;) {
  try {
    await pass();
  } catch (e) {
    log("pass failed:", e.message);
  }
  if (ONCE) break;
  await new Promise((r) => setTimeout(r, INTERVAL_MIN * 60_000));
}
