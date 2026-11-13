// Chain access: views over public RPC, and transactions signed in the browser by the guest key.
import { Account, JsonRpcProvider, KeyPair, actions } from "near-api-js";
import type { KeyPairString } from "near-api-js";
import { actionCreators } from "@near-js/transactions";
import { ARCHIVAL_RPC_URL, FLATRATE_ID, FUSD_ID, RPC_URL, TGAS } from "./config";

export const provider = new JsonRpcProvider({ url: RPC_URL });

export async function view<T>(contractId: string, method: string, args: Record<string, unknown> = {}): Promise<T> {
  const r = await provider.callFunction<T & object>({ contractId, method, args });
  return r as T;
}

export const flat = <T>(method: string, args: Record<string, unknown> = {}) => view<T>(FLATRATE_ID, method, args);

async function rpc<T>(url: string, method: string, params: unknown): Promise<T> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: "flatrate", method, params }),
  });
  const body = await res.json();
  if (body.error) throw new Error(JSON.stringify(body.error));
  return body.result as T;
}

export async function accountExists(accountId: string): Promise<boolean> {
  try {
    await rpc(RPC_URL, "query", { request_type: "view_account", finality: "final", account_id: accountId });
    return true;
  } catch {
    return false;
  }
}

export async function nearBalance(accountId: string): Promise<bigint> {
  const r = await rpc<{ amount: string }>(RPC_URL, "query", {
    request_type: "view_account",
    finality: "final",
    account_id: accountId,
  });
  return BigInt(r.amount);
}

// ---------- transaction results ----------

export type Receipt = {
  executor: string;
  logs: string[];
  success: boolean;
  value?: string; // decoded SuccessValue
  failure?: string;
};

export type TxResult = {
  hash: string;
  ok: boolean; // the transaction's own result
  error?: string; // first ERR_* code or failure text anywhere in the tree
  receipts: Receipt[];
  events: { event: string; data: Record<string, unknown> }[];
};

function decodeB64(v: string): string {
  try {
    return new TextDecoder().decode(Uint8Array.from(atob(v), (c) => c.charCodeAt(0)));
  } catch {
    return v;
  }
}

// deno-lint-ignore no-explicit-any
export function parseOutcome(r: any): TxResult {
  const receipts: Receipt[] = (r.receipts_outcome || []).map((ro: any) => {
    const s = ro.outcome.status || {};
    return {
      executor: ro.outcome.executor_id,
      logs: ro.outcome.logs || [],
      success: !("Failure" in s),
      value: typeof s.SuccessValue === "string" ? decodeB64(s.SuccessValue) : undefined,
      failure: "Failure" in s ? JSON.stringify(s.Failure) : undefined,
    };
  });
  const status = r.status || {};
  const all = JSON.stringify([status, receipts.map((x) => x.failure)]);
  const code = all.match(/ERR_[A-Z_]+/)?.[0];
  const firstFailure = receipts.find((x) => x.failure)?.failure || ("Failure" in status ? JSON.stringify(status.Failure) : undefined);
  const events = receipts
    .flatMap((x) => x.logs)
    .filter((l) => l.startsWith("EVENT_JSON:"))
    .map((l) => {
      try {
        const j = JSON.parse(l.slice("EVENT_JSON:".length));
        return { event: j.event as string, data: (j.data?.[0] || {}) as Record<string, unknown> };
      } catch {
        return null;
      }
    })
    .filter((x): x is { event: string; data: Record<string, unknown> } => !!x);
  return {
    hash: r.transaction?.hash || r.transaction_outcome?.id || "",
    ok: !("Failure" in status),
    error: code || firstFailure,
    receipts,
    events,
  };
}

/** Full receipt tree of a past transaction (archival node). */
export async function txStatus(hash: string, sender: string): Promise<TxResult> {
  const r = await rpc(ARCHIVAL_RPC_URL, "EXPERIMENTAL_tx_status", {
    tx_hash: hash,
    sender_account_id: sender,
    wait_until: "FINAL",
  });
  return parseOutcome(r);
}

// ---------- guest account ----------

const GUEST_KEY = "flatrate.guest.key";

/** Anything that can sign for the user: the guest key in this browser, or a NEAR wallet (extra 6). */
export type WalletSigner = {
  signAndSendTransaction(p: { signerId?: string; receiverId: string; actions: ReturnType<typeof actionCreators.functionCall>[] }): Promise<unknown>;
};
export type Guest = { accountId: string; keyPair?: KeyPairString; account?: Account; wallet?: WalletSigner; walletName?: string };

/** A signed-in wallet account used in place of the guest. */
export const walletUser = (accountId: string, wallet: WalletSigner, walletName: string): Guest => ({ accountId, wallet, walletName });

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The guest key pair lives only in this browser. The account id is the implicit account of the key. */
export function loadOrCreateGuest(): Guest {
  let kp: KeyPairString | null = null;
  try {
    kp = localStorage.getItem(GUEST_KEY) as KeyPairString | null;
  } catch {
    /* storage blocked: a fresh key per page load */
  }
  if (!kp) {
    kp = KeyPair.fromRandom("ed25519").toString();
    try {
      localStorage.setItem(GUEST_KEY, kp);
    } catch {
      /* ignore */
    }
  }
  const pair = KeyPair.fromString(kp);
  const accountId = hex(pair.getPublicKey().data);
  return { accountId, keyPair: kp, account: new Account(accountId, provider, kp) };
}

export function forgetGuest(): void {
  try {
    localStorage.removeItem(GUEST_KEY);
  } catch {
    /* ignore */
  }
}

/** Asks api/sponsor to fund the implicit account (0.5 NEAR + 100 fUSD). */
export async function sponsor(accountId: string): Promise<{ ok: boolean; message: string; txs?: string[] }> {
  const res = await fetch("/api/sponsor", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ account_id: accountId }),
  });
  const body = await res.json().catch(() => ({}));
  return { ok: res.ok, message: body.message || body.error || res.statusText, txs: body.txs };
}

export async function call(
  guest: Guest,
  receiverId: string,
  method: string,
  args: Record<string, unknown>,
  opts: { gasTgas?: number; deposit?: bigint; waitUntil?: "FINAL" | "INCLUDED_FINAL" } = {},
): Promise<TxResult> {
  const gas = BigInt(opts.gasTgas ?? 100) * TGAS;
  if (guest.wallet) {
    const r = await guest.wallet.signAndSendTransaction({
      signerId: guest.accountId,
      receiverId,
      actions: [actionCreators.functionCall(method, args, gas, opts.deposit ?? 0n)],
    });
    // Redirect wallets return nothing here; the result shows up after the page comes back.
    if (!r) return { hash: "", ok: true, error: "sent through the wallet; refresh after it returns", receipts: [], events: [] };
    return parseOutcome(r);
  }
  if (!guest.account) throw new Error("no signer");
  const r = await guest.account.signAndSendTransaction({
    receiverId,
    actions: [actions.functionCall(method, args, gas, opts.deposit ?? 0n)],
    throwOnFailure: false,
    waitUntil: opts.waitUntil ?? "FINAL",
  });
  return parseOutcome(r);
}

export const callFlat = (
  g: Guest,
  method: string,
  args: Record<string, unknown>,
  opts?: { gasTgas?: number; deposit?: bigint; waitUntil?: "FINAL" | "INCLUDED_FINAL" },
) =>
  call(g, FLATRATE_ID, method, args, opts);

/** fUSD `ft_transfer_call` into Flatrate with a JSON message (mint_pair or commit_bid). */
export const transferCall = (g: Guest, amount: bigint, msg: Record<string, unknown>) =>
  call(g, FUSD_ID, "ft_transfer_call", { receiver_id: FLATRATE_ID, amount: amount.toString(), msg: JSON.stringify(msg) }, { gasTgas: 100, deposit: 1n });

// ---------- commitment hash (same bytes as the contract's bid_commitment) ----------

/** sha256(borsh(market_id: u64, side: u8, qty: u64, limit_cents: u8, salt: String)) as hex. */
export async function bidHash(marketId: number, side: "Long" | "Short", qty: bigint, limitCents: number, salt: string): Promise<string> {
  const saltBytes = new TextEncoder().encode(salt);
  const buf = new ArrayBuffer(8 + 1 + 8 + 1 + 4 + saltBytes.length);
  const dv = new DataView(buf);
  let o = 0;
  dv.setBigUint64(o, BigInt(marketId), true);
  o += 8;
  dv.setUint8(o, side === "Long" ? 0 : 1);
  o += 1;
  dv.setBigUint64(o, qty, true);
  o += 8;
  dv.setUint8(o, limitCents);
  o += 1;
  dv.setUint32(o, saltBytes.length, true);
  o += 4;
  new Uint8Array(buf, o).set(saltBytes);
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return hex(new Uint8Array(digest));
}

export function randomSalt(): string {
  return hex(crypto.getRandomValues(new Uint8Array(16)));
}
