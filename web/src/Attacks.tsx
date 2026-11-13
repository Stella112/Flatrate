import { useState } from "react";
import { useApp } from "./App";
import { FLATRATE_ID } from "./config";
import { callFlat, flat, view, type TxResult } from "./near";
import type { Market, NftToken, SlotInfo } from "./types";
import { Card, TxLink } from "./ui";

type Attack = {
  key: string;
  title: string;
  what: string;
  expected: string;
  /** Builds the call from live chain state, or says why it cannot run right now. */
  prepare: (guest: string) => Promise<{ method: string; args: Record<string, unknown>; target: string } | { unavailable: string }>;
};

async function markets(): Promise<Market[]> {
  return flat<Market[]>("list_markets", { limit: 100 });
}

const ATTACKS: Attack[] = [
  {
    key: "fake",
    title: "Fake price",
    what: "Call on_observation directly with a made-up price and a request id the contract never issued.",
    expected: "ERR_NOT_OUTLAYER",
    prepare: async () => {
      const ms = await markets();
      const m = ms.find((x) => x.status === "Open") || ms[ms.length - 1];
      return { method: "on_observation", args: { market_id: m.id, pending_id: "999999999" }, target: `market ${m.id}, pending id 999999999` };
    },
  },
  {
    key: "early",
    title: "Early settlement",
    what: "Call settle on a market whose period has not ended.",
    expected: "ERR_PERIOD_NOT_ENDED",
    prepare: async () => {
      const now = BigInt(Date.now()) * 1_000_000n;
      const m = (await markets()).find((x) => x.status === "Open" && BigInt(x.period_end) > now);
      if (!m) return { unavailable: "No market is open with its period still running right now." };
      return { method: "settle", args: { market_id: m.id }, target: `market ${m.id}` };
    },
  },
  {
    key: "slot",
    title: "Second price in the same slot",
    what: "Call request_observation for the current time slot of a market that already has a price (or a request) for it.",
    expected: "ERR_SLOT_FILLED",
    prepare: async () => {
      for (const m of await markets()) {
        if (m.status !== "Open") continue;
        const s = await flat<SlotInfo>("slot_info", { market_id: m.id });
        if (s.current_slot !== null && (s.current_filled || s.current_pending)) {
          return { method: "request_observation", args: { market_id: m.id }, target: `market ${m.id}, slot ${s.current_slot + 1} (already ${s.current_filled ? "filled" : "requested"})` };
        }
      }
      return { unavailable: "No open market has a price for its current slot right now. The keeper fills each slot early; try again in a minute." };
    },
  },
  {
    key: "steal",
    title: "Steal a note",
    what: "Call redeem_note on a Cover Note that belongs to someone else.",
    expected: "ERR_NOT_OWNER",
    prepare: async (guest) => {
      const tokens = await view<NftToken[]>(FLATRATE_ID, "nft_tokens", { limit: 100 });
      const t = tokens.find((x) => x.owner_id !== guest);
      if (!t) return { unavailable: "No Cover Note owned by another account exists right now." };
      return { method: "redeem_note", args: { token_id: t.token_id }, target: `note #${t.token_id} owned by ${t.owner_id}` };
    },
  },
];

export function AttackPanel() {
  const { guest, demo } = useApp();
  return (
    <Card title="Try to break it">
      <p className="small">
        Each button sends a real testnet transaction from your guest account and shows the contract's answer. Every one should fail
        with the error shown. The guest pays a little gas.
      </p>
      <div className="attacks">
        {ATTACKS.map((a) => (
          <AttackRow key={a.key} a={a} guest={guest.accountId} disabled={demo} run={(method, args) => callFlat(guest, method, args, { gasTgas: method === "request_observation" ? 300 : 50 })} />
        ))}
      </div>
      {demo && <p className="muted small">Disabled in scripted demo mode.</p>}
    </Card>
  );
}

function AttackRow({
  a,
  guest,
  disabled,
  run,
}: {
  a: Attack;
  guest: string;
  disabled: boolean;
  run: (method: string, args: Record<string, unknown>) => Promise<TxResult>;
}) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [res, setRes] = useState<TxResult | null>(null);
  const go = async () => {
    setBusy(true);
    setNote(null);
    setRes(null);
    try {
      const p = await a.prepare(guest);
      if ("unavailable" in p) {
        setNote(p.unavailable);
        return;
      }
      setNote(`Target: ${p.target}`);
      setRes(await run(p.method, p.args));
    } catch (e) {
      setNote(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const blocked = res && !res.ok;
  const asExpected = res?.error?.includes(a.expected);
  return (
    <div className="attack">
      <div>
        <strong>{a.title}</strong>
        <div className="small">{a.what}</div>
        <div className="small muted">
          Expected: <span className="mono">{a.expected}</span>
        </div>
      </div>
      <div className="attack-side">
        <button disabled={disabled || busy} onClick={go}>
          {busy ? "Sending…" : "Try it"}
        </button>
      </div>
      {note && <div className="small muted mono attack-wide">{note}</div>}
      {res && (
        <div className={`tx attack-wide ${blocked ? (asExpected ? "tx-ok" : "tx-fail") : "tx-fail"}`}>
          {blocked
            ? asExpected
              ? `Blocked as expected: ${a.expected}`
              : `Blocked, but with a different error: ${res.error}`
            : "Not blocked: the call succeeded. This would be a bug."}{" "}
          · tx <TxLink hash={res.hash} />
        </div>
      )}
    </div>
  );
}
