// Finds the transaction behind each on-chain observation and reads OutLayer's output from it.
// The list of transactions comes from the public NearBlocks API (v3, no key); every result is then
// read from an archival RPC node and matched to contract state by pending id, so the page shows
// only what the chain says.
import { FLATRATE_ID, NEARBLOCKS_API } from "./config";
import { txStatus } from "./near";
import type { OracleOutput } from "./types";

export type ObservationTx = {
  hash: string;
  signer: string;
  pendingId: string | null;
  recorded: boolean;
  rejectReason: string | null;
  oracle: OracleOutput | null;
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

export async function readObservationTx(hash: string, signer: string): Promise<ObservationTx> {
  const r = await txStatus(hash, signer);
  const req = r.events.find((e) => e.event === "observation_requested");
  const rec = r.events.find((e) => e.event === "observation_recorded");
  const rej = r.events.find((e) => e.event === "observation_rejected");
  let oracle: OracleOutput | null = null;
  for (const x of r.receipts) {
    if (x.executor === "outlayer.testnet" && x.value) {
      try {
        const j = JSON.parse(x.value);
        if (j && Array.isArray(j.sources)) oracle = j;
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
    chargedYocto: charged,
  };
}
