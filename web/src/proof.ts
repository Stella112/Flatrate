// Finds the transaction behind each on-chain observation and reads OutLayer's output from it.
// The list of transactions comes from the public NearBlocks API (v3, no key); every result is then
// read from an archival RPC node and matched to contract state by pending id, so the page shows
// only what the chain says.
import { FLATRATE_ID, NEARBLOCKS_API } from "./config";
import { txStatus } from "./near";
import type { Attestation, OracleOutput } from "./types";

export const OUTLAYER_API = "https://testnet-api.outlayer.ai";

export type ObservationTx = {
  hash: string;
  signer: string;
  pendingId: string | null;
  recorded: boolean;
  rejectReason: string | null;
  oracle: OracleOutput | null;
  /** The oracle's output exactly as OutLayer returned it to the contract (receipt bytes). */
  oracleRaw: string | null;
  chargedYocto: string | null;
};

type NbTxn = { transaction_hash: string; signer_account_id: string; actions: { action: string; method: string | null }[] };

export async function listObservationTxs(maxPages = 5): Promise<{ hash: string; signer: string }[]> {
  const out: { hash: string; signer: string }[] = [];
  let next: string | undefined;
  for (let page = 0; page < maxPages; page++) {
    const url = `${NEARBLOCKS_API}/accounts/${FLATRATE_ID}/txns?limit=100${next ? `&next=${encodeURIComponent(next)}` : ""}`;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`NearBlocks ${res.status}`);
    const body: { data: NbTxn[]; meta?: { next_page?: string } } = await res.json();
    for (const t of body.data || []) {
      if (t.actions?.some((a) => a.method === "request_observation")) out.push({ hash: t.transaction_hash, signer: t.signer_account_id });
    }
    next = body.meta?.next_page;
    if (!next || !body.data?.length) break;
  }
  return out;
}

/** Reads many observation transactions, a few at a time so the RPC node does not rate-limit the page. */
export async function readObservationTxs(list: { hash: string; signer: string }[], concurrency = 2): Promise<(ObservationTx | null)[]> {
  const out: (ObservationTx | null)[] = new Array(list.length).fill(null);
  let next = 0;
  const worker = async () => {
    while (next < list.length) {
      const i = next++;
      out[i] = await readObservationTx(list[i].hash, list[i].signer).catch(() => null);
    }
  };
  await Promise.all(Array.from({ length: Math.min(concurrency, list.length) }, worker));
  return out;
}

export async function readObservationTx(hash: string, signer: string): Promise<ObservationTx> {
  const r = await txStatus(hash, signer);
  const req = r.events.find((e) => e.event === "observation_requested");
  const rec = r.events.find((e) => e.event === "observation_recorded");
  const rej = r.events.find((e) => e.event === "observation_rejected");
  let oracle: OracleOutput | null = null;
  let oracleRaw: string | null = null;
  for (const x of r.receipts) {
    if (x.executor === "outlayer.testnet" && x.value) {
      try {
        const j = JSON.parse(x.value);
        if (j && Array.isArray(j.sources)) {
          oracle = j;
          oracleRaw = x.value;
        }
      } catch {
        /* other outlayer receipts */
      }
    }
  }
  const charged = r.receipts.flatMap((x) => x.logs).map((l) => l.match(/yNEAR charged: "(\d+)"/)?.[1]).find(Boolean) || null;
  return {
    hash,
    signer,
    pendingId: (req?.data.pending_id as string) ?? null,
    recorded: !!rec,
    rejectReason: (rej?.data.reason as string) ?? null,
    oracle,
    oracleRaw,
    chargedYocto: charged,
  };
}

export async function sha256Hex(text: string): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(d), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** OutLayer's attestation record for an execution, by its request id (public, CORS *). */
export async function fetchAttestation(requestId: string): Promise<Attestation | null> {
  const r = await fetch(`${OUTLAYER_API}/attestations/by-request/${encodeURIComponent(requestId)}`);
  if (!r.ok) return null;
  return r.json();
}

export type AttestationCheck = {
  att: Attestation;
  wasmMatches: boolean;
  inputMatches: boolean;
  outputMatches: boolean | null; // null when the receipt bytes are not available
};

/** Compares the attestation with the market's pinned version and input, and with the bytes the contract received. */
export async function checkAttestation(
  requestId: string,
  pinnedVersion: string,
  oracleInput: string,
  oracleRaw: string | null,
): Promise<AttestationCheck | null> {
  const att = await fetchAttestation(requestId);
  if (!att) return null;
  return {
    att,
    wasmMatches: att.wasm_hash === pinnedVersion && att.executed_wasm_sha256 === pinnedVersion,
    inputMatches: att.input_hash === (await sha256Hex(oracleInput)),
    outputMatches: oracleRaw === null ? null : att.output_hash === (await sha256Hex(oracleRaw)),
  };
}
