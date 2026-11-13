import { useState } from "react";
import { useApp } from "../App";
import { demoQuote } from "../demo";
import { Card, Field, useAsync } from "../ui";

type Token = { assetId: string; blockchain: string; symbol: string; decimals: number; price: number };
type Quote = { amountIn: string; amountInFormatted: string; amountOutFormatted: string; timeEstimate: number };
type Provider = "1click" | "aurora";
type Result = { quote: Quote | null; error: string | null; recorded?: boolean };

const CHAIN: Record<string, string> = { eth: "Ethereum", arb: "Arbitrum", base: "Base", gnosis: "Gnosis", pol: "Polygon", avax: "Avalanche", bsc: "BNB Chain" };
const NAME: Record<Provider, string> = { "1click": "NEAR Intents 1Click", aurora: "Aurora Intents" };

export function PayPage() {
  const { demo } = useApp();
  const info = useAsync(async () => {
    if (demo) return { tokens: [] as Token[], providers: { "1click": true, aurora: false } };
    const r = await fetch("/api/quote");
    const b = await r.json();
    if (!r.ok) throw new Error(b.error || r.statusText);
    return b as { tokens: Token[]; providers: Record<Provider, boolean> };
  }, [demo]);
  const [asset, setAsset] = useState("");
  const [amount, setAmount] = useState("100");
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<Partial<Record<Provider, Result>>>(
    demo ? { "1click": { quote: demoQuote.quote as Quote, error: null, recorded: true } } : {},
  );
  const tokens = info.data?.tokens || [];
  const selected = tokens.find((t) => t.assetId === (asset || tokens.find((x) => x.blockchain === "base")?.assetId));

  const ask = async (provider: Provider, raw: string): Promise<Result> => {
    try {
      const r = await fetch("/api/quote", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, originAsset: selected!.assetId, amount: raw }),
      });
      const b = await r.json();
      if (r.ok && b.quote) return { quote: b.quote, error: null };
      const msg = b?.error?.message || (typeof b?.error === "string" ? b.error : JSON.stringify(b.error || b));
      return { quote: null, error: msg };
    } catch (e) {
      return { quote: null, error: e instanceof Error ? e.message : String(e) };
    }
  };

  const getQuotes = async () => {
    if (!selected) return;
    const m = amount.trim().match(/^(\d+)(?:\.(\d+))?$/);
    if (!m) return;
    const frac = (m[2] || "").slice(0, selected.decimals).padEnd(selected.decimals, "0");
    const raw = (BigInt(m[1]) * 10n ** BigInt(selected.decimals) + BigInt(frac || "0")).toString();
    setBusy(true);
    setResults({});
    const [a, b] = await Promise.all([ask("1click", raw), ask("aurora", raw)]);
    setResults({ "1click": a, aurora: b });
    setBusy(false);
  };

  return (
    <Card title="Pay from any chain" right={<span className="pill pill-warn">quote only on testnet</span>}>
      <p className="small">
        On mainnet a buyer could pay in from another chain through NEAR Intents. Neither NEAR Intents 1Click nor the Aurora Intents
        API has a testnet, so this page asks both for a <strong>dry-run quote</strong> (<span className="mono">dry: true</span>) at
        today's mainnet prices: what it would cost to send USDC from an EVM chain and receive USDC on NEAR. Nothing is sent and no
        deposit address is created. API keys stay on the server.
      </p>
      {demo ? (
        <p className="small muted">Scripted demo: showing the 1Click quote recorded on 2026-11-12 (fixtures/1click-quote.json).</p>
      ) : (
        <div className="form">
          <Field label="From">
            <select value={selected?.assetId || ""} onChange={(e) => setAsset(e.target.value)}>
              {tokens.map((t) => (
                <option key={t.assetId} value={t.assetId}>
                  USDC on {CHAIN[t.blockchain] || t.blockchain}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Amount (USDC)">
            <input value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
          </Field>
          <div className="form-actions">
            <button disabled={busy || !selected} onClick={getQuotes}>
              {busy ? "Asking both…" : "Get both quotes"}
            </button>
          </div>
        </div>
      )}
      {info.error && <p className="err">{info.error}</p>}
      <div className="quotes">
        {(["1click", "aurora"] as Provider[]).map((p) => (
          <QuoteBox key={p} name={NAME[p]} r={results[p]} configured={info.data?.providers?.[p] ?? true} busy={busy} />
        ))}
      </div>
    </Card>
  );
}

function QuoteBox({ name, r, configured, busy }: { name: string; r?: Result; configured: boolean; busy: boolean }) {
  return (
    <div className="quote">
      <h3>{name}</h3>
      {!configured && <p className="small muted">Not configured on this deployment.</p>}
      {busy && <p className="small muted">Waiting…</p>}
      {r?.error && <p className="small err">No quote: {r.error}</p>}
      {r?.quote && (
        <>
          {r.recorded && <p className="small"><span className="pill pill-warn">recorded</span> not a live price</p>}
          <div className="grid4">
            <div className="stat">
              <small className="muted">You send</small>
              <div>{r.quote.amountInFormatted} USDC</div>
            </div>
            <div className="stat">
              <small className="muted">You receive on NEAR</small>
              <div>{r.quote.amountOutFormatted} USDC</div>
            </div>
            <div className="stat">
              <small className="muted">Cost</small>
              <div>{(Number(r.quote.amountInFormatted) - Number(r.quote.amountOutFormatted)).toFixed(4)} USDC</div>
            </div>
            <div className="stat">
              <small className="muted">Estimated time</small>
              <div>{r.quote.timeEstimate} s</div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
