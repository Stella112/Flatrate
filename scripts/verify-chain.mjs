#!/usr/bin/env node
// Read-only checks of the deployed testnet contracts (view calls over public RPC; no keys).
// Prints one line per check: "CHECK<TAB>STATUS<TAB>EVIDENCE" with STATUS PASS, FAIL, SKIP or PENDING.
// Usage: node scripts/verify-chain.mjs [--oracle-hash <sha256 of a reproducible build of oracle/>]
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const OUTLAYER_ID = process.env.OUTLAYER_ID || "outlayer.testnet";
const PAIR = 1_000_000n;

const argHash = (() => {
  const i = process.argv.indexOf("--oracle-hash");
  return i > 0 ? process.argv[i + 1] : "";
})();

async function rpc(method, params) {
  for (let i = 1; ; i++) {
    try {
      const r = await fetch(RPC, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "verify", method, params }),
      });
      const b = await r.json();
      if (b.error) throw new Error(JSON.stringify(b.error));
      return b.result;
    } catch (e) {
      if (i >= 4) throw e;
      await new Promise((r) => setTimeout(r, 1500 * i));
    }
  }
}
async function view(account, method, args = {}) {
  const r = await rpc("query", {
    request_type: "call_function",
    finality: "final",
    account_id: account,
    method_name: method,
    args_base64: Buffer.from(JSON.stringify(args)).toString("base64"),
  });
  return JSON.parse(Buffer.from(r.result).toString());
}

// Same rules as shared/src/lib.rs.
const mean = (xs) => xs.reduce((a, b) => a + b, 0n) / BigInt(xs.length);
function payoff(avg, floor, cap) {
  if (avg <= floor) return 0n;
  if (avg >= cap) return PAIR;
  return ((avg - floor) * PAIR) / (cap - floor);
}

const rows = [];
const add = (check, status, evidence) => rows.push([check, status, evidence]);

try {
  const cfg = await view(FLATRATE_ID, "get_config");
  const markets = await view(FLATRATE_ID, "list_markets", { from_index: "0", limit: 100 });
  add("contract readable", "PASS", `${FLATRATE_ID}: ${markets.length} markets, owner ${cfg.owner_id}, oracle ${cfg.oracle_project_id}`);

  // 1. The version pinned for new markets is the reproducible build of oracle/ at this commit.
  if (argHash) {
    add(
      "pinned oracle version = build of oracle/",
      cfg.oracle_version_key === argHash ? "PASS" : "FAIL",
      `contract ${cfg.oracle_version_key.slice(0, 16)}… vs local reproducible build ${argHash.slice(0, 16)}…`,
    );
  } else {
    add("pinned oracle version = build of oracle/", "SKIP", "no reproducible build hash (needs docker): bash scripts/oracle-reproducible.sh");
  }

  // 2. Every version pinned by a market is registered on OutLayer as the wasm with that sha256.
  const versions = [...new Set(markets.map((m) => m.oracle_version_key).concat([cfg.oracle_version_key]))];
  for (const v of versions) {
    try {
      const ver = await view(OUTLAYER_ID, "get_version", { project_id: cfg.oracle_project_id, version_key: v });
      const used = markets.filter((m) => m.oracle_version_key === v).map((m) => m.id);
      add(
        `OutLayer version ${v.slice(0, 10)}… exists`,
        ver && ver.wasm_hash === v ? "PASS" : "FAIL",
        `wasm_hash ${ver?.wasm_hash?.slice(0, 16)}…, markets [${used.join(",")}]${v === cfg.oracle_version_key ? " + new markets" : ""}`,
      );
    } catch (e) {
      add(`OutLayer version ${v.slice(0, 10)}… exists`, "FAIL", String(e).slice(0, 120));
    }
  }

  // 3. Every settled market pays what the payoff of its stored observations says.
  for (const m of markets) {
    const obs = m.observations.map((o) => BigInt(o.price_micros));
    if (typeof m.status === "object" && m.status.Settled) {
      const want = payoff(mean(obs), BigInt(m.floor_micros), BigInt(m.cap_micros));
      const got = BigInt(m.status.Settled.long_payout_micros);
      add(
        `market ${m.id} payout recomputed`,
        want === got && obs.length >= m.min_observations ? "PASS" : "FAIL",
        `${obs.length} obs, mean ${mean(obs)}, long ${got} (recomputed ${want})`,
      );
    } else if (m.status === "Void") {
      add(`market ${m.id} void is justified`, obs.length < m.min_observations ? "PASS" : "FAIL", `${obs.length} obs < min ${m.min_observations}`);
    }
  }

  // 4. Keyless: no full-access key on the contract account.
  const keys = await rpc("query", { request_type: "view_access_key_list", finality: "final", account_id: FLATRATE_ID });
  const full = keys.keys.filter((k) => k.access_key.permission === "FullAccess");
  add(
    "contract has no full-access key",
    full.length === 0 ? "PASS" : "PENDING",
    full.length === 0 ? `${keys.keys.length} keys, none full-access` : `${full.length} full-access key(s); deleted only after Maris writes "go keyless" (CLAUDE.md Rule 5)`,
  );
} catch (e) {
  add("chain checks", "FAIL", String(e).slice(0, 200));
}

for (const r of rows) console.log(r.join("\t"));
process.exit(rows.some((r) => r[1] === "FAIL") ? 1 : 0);
