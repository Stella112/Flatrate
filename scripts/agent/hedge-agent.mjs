#!/usr/bin/env node
// Flatrate hedging agent (extra 1). It reads a usage forecast and, for each new market, places a
// sealed Long bid sized to hedge part of its expected GPU-hours, then reveals, clears, withdraws
// unused escrow and redeems after settlement.
//
// Keys (both on the agent's own account, both outside the repository):
//   AGENT_COMMIT_KEY  full-access key file. Used ONLY to commit a bid: the commit is an fUSD
//                     ft_transfer_call that needs 1 yoctoNEAR attached, which a function-call key
//                     cannot attach (BLOCKERS.md B11). The limit here is the account's budget: it
//                     holds only the fUSD it may spend, and the forecast's max_spend_fusd caps
//                     each bid.
//   AGENT_KEY         function-call key limited to the flatrate contract. Used for everything
//                     else: reveal_bid, clear_auction, withdraw_refund, redeem, roll_market.
//
// Usage: AGENT_ACCOUNT=agent.flatrate-maris.testnet \
//        AGENT_COMMIT_KEY=~/.near-credentials/testnet/agent.flatrate-maris.testnet/commit-key.json \
//        AGENT_KEY=~/.near-credentials/testnet/agent.flatrate-maris.testnet/hedge-key.json \
//        node scripts/agent/hedge-agent.mjs [--once] [--dry-run]
// Env:   AGENT_FORECAST (scripts/agent/forecast.example.json), AGENT_TEMPLATES ("DEMO"),
//        AGENT_ROLL (templates the agent may roll when the latest is done, default ""),
//        AGENT_STATE (target/agent-state.json, holds salts; gitignored), AGENT_INTERVAL_SEC (30),
//        FLATRATE_ID, FUSD_ID, NEAR_RPC.
// --dry-run prints the bid it would place for the latest market and sends nothing.

import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { randomBytes } from "node:crypto";
import { homedir } from "node:os";
import { dirname } from "node:path";
import { planBid, referencePrice } from "./plan.mjs";

const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const FUSD_ID = process.env.FUSD_ID || "fusd.flatrate-maris.testnet";
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const ACCOUNT = process.env.AGENT_ACCOUNT;
const home = (p) => p?.replace(/^~(?=\/|\\)/, homedir());
const COMMIT_KEY = home(process.env.AGENT_COMMIT_KEY);
const FC_KEY = home(process.env.AGENT_KEY);
const FORECAST_FILE = process.env.AGENT_FORECAST || "scripts/agent/forecast.example.json";
const TEMPLATES = (process.env.AGENT_TEMPLATES ?? "DEMO").split(",").map((s) => s.trim()).filter(Boolean);
const ROLL = (process.env.AGENT_ROLL ?? "").split(",").map((s) => s.trim()).filter(Boolean);
const STATE_FILE = process.env.AGENT_STATE || "target/agent-state.json";
const INTERVAL_SEC = Number(process.env.AGENT_INTERVAL_SEC || 30);
const ONCE = process.argv.includes("--once");
const DRY = process.argv.includes("--dry-run");

const log = (...a) => console.log(new Date().toISOString(), ...a);
if (!ACCOUNT) { console.error("Set AGENT_ACCOUNT."); process.exit(2); }
if (!DRY && (!COMMIT_KEY || !FC_KEY)) { console.error("Set AGENT_COMMIT_KEY and AGENT_KEY (key files)."); process.exit(2); }

async function view(contract, method, args = {}) {
  for (let attempt = 1; ; attempt++) {
    try {
      const res = await fetch(RPC, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0", id: "agent", method: "query",
          params: { request_type: "call_function", finality: "optimistic", account_id: contract, method_name: method,
            args_base64: Buffer.from(JSON.stringify(args)).toString("base64") },
        }),
      });
      const body = await res.json();
      if (body.error || body.result?.error) throw new Error(JSON.stringify(body.error || body.result.error));
      return JSON.parse(Buffer.from(body.result.result).toString());
    } catch (e) {
      if (attempt >= 4) throw e;
      await new Promise((r) => setTimeout(r, 3000 * attempt));
    }
  }
}

/** One signed call through near-cli-rs with an access-key file. */
function call(keyFile, contract, method, args, { gas = 100, deposit = "0 NEAR" } = {}) {
  let out = "";
  for (let attempt = 1; attempt <= 3; attempt++) {
    const r = spawnSync("near", [
      "contract", "call-function", "as-transaction", contract, method, "json-args", JSON.stringify(args),
      "prepaid-gas", `${gas} Tgas`, "attached-deposit", deposit, "sign-as", ACCOUNT, "network-config", "testnet",
      "sign-with-access-key-file", keyFile, "send",
    ], { encoding: "utf8", input: "" });
    out = (r.stdout + r.stderr).replace(/\x1b\[[0-9;]*m/g, "");
    if (/Transaction ID/.test(out) || !/error sending request|rate limit/i.test(out)) break;
    log(`${method}: nothing sent (network), retry ${attempt}`);
    spawnSync(process.platform === "win32" ? "timeout" : "sleep", process.platform === "win32" ? ["/t", "10", "/nobreak"] : ["10"]);
  }
  const tx = out.match(/Transaction ID: (\w+)/)?.[1] || null;
  // near-cli prints "Error:" then the reason on the next numbered line ("0: ..."); when nothing was sent there is no tx id.
  const cliErr = out.match(/^\s*0: (.+)$/m)?.[1]?.trim();
  const err = out.match(/ERR_[A-Z_]+/)?.[0] || (!tx ? cliErr || "no transaction sent" : null);
  const events = [...out.matchAll(/EVENT_JSON:(\{.*\})/g)].map((m) => { try { return JSON.parse(m[1]); } catch { return null; } }).filter(Boolean);
  return { tx, err, events };
}

const loadState = () => (existsSync(STATE_FILE) ? JSON.parse(readFileSync(STATE_FILE, "utf8")) : { markets: {} });
function saveState(s) { mkdirSync(dirname(STATE_FILE), { recursive: true }); writeFileSync(STATE_FILE, JSON.stringify(s, null, 2)); }
const isDone = (m) => m.status === "Void" || (typeof m.status === "object" && m.status?.Settled);

async function previousOf(m) {
  const all = await view(FLATRATE_ID, "list_markets", { limit: 100 });
  return all.filter((x) => x.template === m.template && BigInt(x.id) < BigInt(m.id)).sort((a, b) => Number(BigInt(b.id) - BigInt(a.id)))[0] || null;
}

async function commit(m, state) {
  const forecast = JSON.parse(readFileSync(FORECAST_FILE, "utf8"));
  const ref = referencePrice(m, await previousOf(m), forecast);
  const balance = BigInt(await view(FUSD_ID, "ft_balance_of", { account_id: ACCOUNT }));
  const plan = planBid(m, ref.priceMicros, forecast, balance);
  if (plan.skip) { log(`market ${m.id}: skip (${plan.skip})`); if (!DRY) state.markets[m.id] = { skipped: plan.skip }; return; }
  const summary = `Long ${plan.qty} @ <= ${plan.limitCents}c (fair ${plan.fairCents}c from ${ref.source} ${ref.priceMicros}), cost ${plan.costMicros}, escrow ${plan.escrowMicros}`;
  if (DRY) { log(`market ${m.id}: would bid ${summary}`); return; }
  const salt = randomBytes(16).toString("hex");
  const bid = { market_id: m.id, side: "Long", qty: plan.qty.toString(), limit_cents: plan.limitCents, salt };
  const hash = await view(FLATRATE_ID, "bid_hash", bid);
  // Save the salt before sending, so a crash after the commit can still reveal.
  state.markets[m.id] = { ...bid, escrow: plan.escrowMicros.toString(), hash, reason: summary };
  saveState(state);
  const r = call(COMMIT_KEY, FUSD_ID, "ft_transfer_call", {
    receiver_id: FLATRATE_ID, amount: plan.escrowMicros.toString(),
    msg: JSON.stringify({ action: "commit_bid", market_id: m.id, hash }),
  }, { deposit: "1 yoctoNEAR" });
  state.markets[m.id].commit_tx = r.tx;
  if (r.err) state.markets[m.id].commit_err = r.err;
  log(`market ${m.id}: commit tx ${r.tx} ${r.err || ""} | ${summary}`);
}

async function pass() {
  const state = loadState();
  const now = BigInt(Date.now()) * 1_000_000n;

  for (const t of TEMPLATES) {
    const id = await view(FLATRATE_ID, "latest_market", { template: t });
    if (!id) continue;
    let m = await view(FLATRATE_ID, "get_market", { market_id: id });
    if (isDone(m) && ROLL.includes(t) && !DRY) {
      const r = call(FC_KEY, FLATRATE_ID, "roll_market", { template: t }, { gas: 50 });
      log(`${t}: roll_market tx ${r.tx} ${r.err || ""}`);
      const nid = await view(FLATRATE_ID, "latest_market", { template: t });
      m = await view(FLATRATE_ID, "get_market", { market_id: nid });
    }
    if (m.status === "Auction" && now < BigInt(m.auction_commit_end) && !state.markets[m.id]) await commit(m, state);
    else if (DRY) log(`market ${m.id}: not in its commit phase (status ${JSON.stringify(m.status)})`);
  }
  if (DRY) return;

  for (const [id, b] of Object.entries(state.markets)) {
    if (b.skipped || b.done || !b.commit_tx) continue;
    const m = await view(FLATRATE_ID, "get_market", { market_id: id });
    const t = BigInt(Date.now()) * 1_000_000n;
    if (m.status === "Auction" && !b.revealed && t >= BigInt(m.auction_commit_end) && t < BigInt(m.auction_reveal_end)) {
      const r = call(FC_KEY, FLATRATE_ID, "reveal_bid", { market_id: id, side: b.side, qty: b.qty, limit_cents: b.limit_cents, salt: b.salt }, { gas: 30 });
      log(`market ${id}: reveal tx ${r.tx} ${r.err || ""}`);
      if (!r.err && r.tx) b.revealed = r.tx;
    } else if (m.status === "Auction" && t >= BigInt(m.auction_reveal_end)) {
      // The keeper (or anyone) may clear first; then this call fails and the next pass sees the market open.
      const r = call(FC_KEY, FLATRATE_ID, "clear_auction", { market_id: id }, { gas: 300 });
      log(`market ${id}: clear_auction ${r.tx ? `tx ${r.tx}` : "not sent"} ${r.err || ""}`);
    }
    if (m.status !== "Auction" && BigInt(await view(FLATRATE_ID, "refund_balance_of", { account_id: ACCOUNT })) > 0n) {
      const r = call(FC_KEY, FLATRATE_ID, "withdraw_refund", {});
      log(`market ${id}: withdraw_refund tx ${r.tx} ${r.err || ""}`);
      if (r.tx && !r.err) b.refund_tx = r.tx;
    }
    if (m.status !== "Auction" && !b.filled) {
      b.filled = (await view(FLATRATE_ID, "claim_balance_of", { account_id: ACCOUNT, market_id: id, side: "Long" }));
      b.opening_price_cents = m.opening_price_cents;
      log(`market ${id}: opened at ${m.opening_price_cents}c, agent holds ${b.filled} Long`);
    }
    if (isDone(m)) {
      const long = BigInt(await view(FLATRATE_ID, "claim_balance_of", { account_id: ACCOUNT, market_id: id, side: "Long" }));
      if (long > 0n) {
        const r = call(FC_KEY, FLATRATE_ID, "redeem", { market_id: id });
        log(`market ${id}: redeem tx ${r.tx} ${r.err || ""}`);
        if (r.tx && !r.err) { b.redeem_tx = r.tx; b.done = true; }
      } else b.done = true;
    }
  }
  saveState(state);
}

for (;;) {
  try { await pass(); } catch (e) { log("pass failed:", e.message); }
  if (ONCE || DRY) break;
  await new Promise((r) => setTimeout(r, INTERVAL_SEC * 1000));
}
