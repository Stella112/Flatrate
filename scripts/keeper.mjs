#!/usr/bin/env node
// Flatrate keeper. Anyone can run it; every call it makes is public and needs no deposit (the
// contract pays OutLayer from its observation budget), so it runs on a function-call access key
// limited to the flatrate contract (methods: clear_auction, request_observation, settle,
// roll_market).
//
// One pass, for every market:
//   - Auction past its reveal window      -> clear_auction
//   - Open, inside its period             -> request_observation for the current slot if that slot
//                                            has no price and no request in flight; one retry if
//                                            the result was rejected (the slot is then free again)
//   - Open, past its period               -> settle (when enough slots are filled, or once void_after passed)
// and for each template in KEEPER_ROLL (default WEEKLY): roll_market when the latest is done.
//
// View calls use public JSON-RPC; transactions are signed by near-cli-rs with the key in
// ~/.near-credentials (legacy keychain).
//
// Usage:  KEEPER_ACCOUNT=<account> node scripts/keeper.mjs [--once]   (loop: every KEEPER_INTERVAL_SEC, default 60)
// Env:    FLATRATE_ID (flatrate.flatrate-maris.testnet), NEAR_RPC (https://rpc.testnet.fastnear.com),
//         KEEPER_ROLL (comma list of templates to roll, default "WEEKLY"; "" to roll none)

import { spawnSync } from "node:child_process";

const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const ACCOUNT = process.env.KEEPER_ACCOUNT;
const INTERVAL_SEC = Number(process.env.KEEPER_INTERVAL_SEC || 60);
const ROLL = (process.env.KEEPER_ROLL ?? "WEEKLY").split(",").map((s) => s.trim()).filter(Boolean);
const ONCE = process.argv.includes("--once");

if (!ACCOUNT) {
  console.error("Set KEEPER_ACCOUNT to the account whose key signs the calls.");
  process.exit(2);
}

const log = (...a) => console.log(new Date().toISOString(), ...a);

async function viewOnce(method, args) {
  const res = await fetch(RPC, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: "keeper",
      method: "query",
      params: {
        request_type: "call_function",
        finality: "optimistic",
        account_id: FLATRATE_ID,
        method_name: method,
        args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
      },
    }),
  });
  const body = await res.json();
  if (body.error || body.result?.error) throw new Error(JSON.stringify(body.error || body.result.error));
  return JSON.parse(Buffer.from(body.result.result).toString());
}

// Retries transient network errors (public RPCs drop requests now and then).
async function view(method, args = {}) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await viewOnce(method, args);
    } catch (e) {
      if (attempt >= 4) throw e;
      log(`view ${method} failed (${e.message}), retry ${attempt}`);
      await new Promise((r) => setTimeout(r, 3_000 * attempt));
    }
  }
}

/** Signs and sends one call; returns the tx id, events and the first ERR_* code if any. */
function call(method, args, gasTgas = 100) {
  const r = spawnSync(
    "near",
    [
      "contract", "call-function", "as-transaction", FLATRATE_ID, method, "json-args", JSON.stringify(args),
      "prepaid-gas", `${gasTgas} Tgas`, "attached-deposit", "0 NEAR",
      "sign-as", ACCOUNT, "network-config", "testnet", "sign-with-legacy-keychain", "send",
    ],
    { encoding: "utf8", input: "" },
  );
  const out = (r.stdout + r.stderr).replace(/\x1b\[[0-9;]*m/g, "");
  const tx = out.match(/Transaction ID: (\w+)/)?.[1];
  const events = [...out.matchAll(/EVENT_JSON:(\{.*\})/g)].map((m) => {
    try {
      return JSON.parse(m[1]);
    } catch {
      return null;
    }
  }).filter(Boolean);
  const err = out.match(/ERR_[A-Z_]+/)?.[0] || (r.status === 0 ? null : out.split("\n").filter(Boolean).slice(-2).join(" "));
  return { tx, events, err };
}

const ev = (r, name) => r.events.find((e) => e.event === name);

async function observe(m) {
  for (let attempt = 1; attempt <= 2; attempt++) {
    const s = await view("slot_info", { market_id: m.id });
    if (s.current_slot === null || s.current_filled || s.current_pending) return;
    const r = call("request_observation", { market_id: m.id }, 300);
    const rec = ev(r, "observation_recorded");
    const rej = ev(r, "observation_rejected");
    log(`market ${m.id} slot ${s.current_slot}: tx ${r.tx} ${rec ? `recorded ${rec.data[0].price_micros} (OutLayer ${rec.data[0].outlayer_request})` : rej ? `rejected: ${rej.data[0].reason}` : r.err || "no event"}`);
    if (!rej) return; // recorded, or failed before OutLayer: do not retry
  }
}

async function pass() {
  const nowNs = BigInt(Date.now()) * 1_000_000n;
  const markets = await view("list_markets", { limit: 100 });
  for (const m of markets) {
    if (m.status === "Auction" && nowNs >= BigInt(m.auction_reveal_end)) {
      const r = call("clear_auction", { market_id: m.id }, 300);
      log(`market ${m.id}: clear_auction tx ${r.tx} ${r.err || ""}`);
      m.status = r.err ? m.status : "Open";
    }
    if (m.status === "Open" && nowNs >= BigInt(m.period_start) && nowNs < BigInt(m.period_end)) {
      await observe(m);
    } else if (m.status === "Open" && nowNs >= BigInt(m.period_end)) {
      if (m.filled_slots >= m.min_filled_slots || nowNs >= BigInt(m.void_after)) {
        const r = call("settle", { market_id: m.id });
        const s = ev(r, "market_settled") || ev(r, "market_void");
        log(`market ${m.id}: settle tx ${r.tx} ${s ? s.event : r.err || ""}`);
      }
    }
  }
  for (const t of ROLL) {
    const latest = await view("latest_market", { template: t });
    if (!latest) continue;
    const m = await view("get_market", { market_id: latest });
    if (m && (m.status === "Void" || (typeof m.status === "object" && m.status.Settled))) {
      const r = call("roll_market", { template: t }, 50);
      log(`${t}: roll_market tx ${r.tx} ${ev(r, "market_rolled") ? JSON.stringify(ev(r, "market_rolled").data[0]) : r.err || ""}`);
    }
  }
}

for (;;) {
  try {
    await pass();
  } catch (e) {
    log("pass failed:", e.message);
  }
  if (ONCE) break;
  await new Promise((r) => setTimeout(r, INTERVAL_SEC * 1000));
}
