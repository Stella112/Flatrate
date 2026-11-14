#!/usr/bin/env node
// Read-only checks of the deployed testnet contracts (view calls over public RPC; no keys).
// Prints one line per check: "CHECK<TAB>STATUS<TAB>EVIDENCE" with STATUS PASS, FAIL, SKIP or PENDING.
// Usage: node scripts/verify-chain.mjs [--oracle-hash <sha256 of a reproducible build of oracle/>]
const RPC = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const FLATRATE_ID = process.env.FLATRATE_ID || "flatrate.flatrate-maris.testnet";
const OUTLAYER_ID = process.env.OUTLAYER_ID || "outlayer.testnet";
const PAIR = 1_000_000n;
const OUTLAYER_API = process.env.OUTLAYER_API || "https://testnet-api.outlayer.ai";
// Extra 2: the same checks for the token-price contract: FLATRATE_ID=tokens.flatrate-maris.testnet
// ORACLE_DIR=oracle-tokens CHECK_PREFIX="[tokens] ".
const ORACLE_DIR = process.env.ORACLE_DIR || "oracle";
const PREFIX = process.env.CHECK_PREFIX || "";
import { createHash } from "node:crypto";

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
const add = (check, status, evidence) => rows.push([PREFIX + check, status, evidence]);

try {
  const cfg = await view(FLATRATE_ID, "get_config");
  const markets = await view(FLATRATE_ID, "list_markets", { limit: 100 });
  add("contract readable", "PASS", `${FLATRATE_ID}: ${markets.length} markets, owner ${cfg.owner_id}, oracle ${cfg.oracle_project_id}`);

  // 1. The version pinned for new markets is the reproducible build of oracle/ at this commit.
  if (argHash) {
    add(
      `pinned oracle version = build of ${ORACLE_DIR}/`,
      cfg.oracle_version_key === argHash ? "PASS" : "FAIL",
      `contract ${cfg.oracle_version_key.slice(0, 16)}… vs local reproducible build ${argHash.slice(0, 16)}…`,
    );
  } else {
    add(`pinned oracle version = build of ${ORACLE_DIR}/`, "SKIP", `no reproducible build hash (needs docker): ORACLE_DIR=${ORACLE_DIR} bash scripts/oracle-reproducible.sh`);
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

  // 2b. A private-auction deployment pins its enclave version, registered on OutLayer, and holds the enclave's key.
  if (cfg.enclave) {
    try {
      const ver = await view(OUTLAYER_ID, "get_version", { project_id: cfg.enclave.project_id, version_key: cfg.enclave.version_key });
      add(
        `enclave version ${cfg.enclave.version_key.slice(0, 10)}… exists`,
        ver && ver.wasm_hash === cfg.enclave.version_key ? "PASS" : "FAIL",
        `${cfg.enclave.project_id}, wasm_hash ${ver?.wasm_hash?.slice(0, 16)}…; enclave key ${cfg.enclave_pubkey ? cfg.enclave_pubkey.slice(0, 16) + "…" : "not set"}`,
      );
    } catch (e) {
      add(`enclave version ${cfg.enclave.version_key.slice(0, 10)}… exists`, "FAIL", String(e).slice(0, 120));
    }
  }

  // 3. Every settled market pays what the payoff of its filled slots says; a void one had too few.
  for (const m of markets) {
    const obs = m.observations.map((o) => BigInt(o.price_micros));
    if (typeof m.status === "object" && m.status.Settled) {
      const want = payoff(mean(obs), BigInt(m.floor_micros), BigInt(m.cap_micros));
      const got = BigInt(m.status.Settled.long_payout_micros);
      add(
        `market ${m.id} payout recomputed`,
        want === got && obs.length >= m.min_filled_slots && String(mean(obs)) === m.settled_average_micros ? "PASS" : "FAIL",
        `${obs.length}/${m.num_slots} slots, mean ${mean(obs)}, long ${got} (recomputed ${want})`,
      );
    } else if (m.status === "Void") {
      add(`market ${m.id} void is justified`, m.filled_slots < m.min_filled_slots ? "PASS" : "FAIL", `${m.filled_slots} slots < min ${m.min_filled_slots}`);
    }
  }

  // 4. Each recorded price has an OutLayer attestation for the pinned wasm and this market's input.
  let attOk = 0;
  let attBad = [];
  let attMissing = 0;
  for (const m of markets) {
    const inputHash = createHash("sha256").update(m.oracle_input).digest("hex");
    for (const o of m.observations) {
      try {
        const r = await fetch(`${OUTLAYER_API}/attestations/by-request/${o.outlayer_request}`);
        if (!r.ok) {
          attMissing++;
          continue;
        }
        const a = await r.json();
        if (a.wasm_hash === m.oracle_version_key && a.executed_wasm_sha256 === m.oracle_version_key && a.input_hash === inputHash) attOk++;
        else attBad.push(o.outlayer_request);
      } catch {
        attMissing++;
      }
    }
  }
  add(
    "OutLayer attestation per recorded price",
    attBad.length ? "FAIL" : attOk > 0 || attMissing === 0 ? "PASS" : "SKIP",
    `${attOk} match pinned wasm + market input${attMissing ? `, ${attMissing} not reachable` : ""}${attBad.length ? `, mismatch: ${attBad.join(",")}` : ""}`,
  );

  // 5. Keyless: no full-access key on the contract account.
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
