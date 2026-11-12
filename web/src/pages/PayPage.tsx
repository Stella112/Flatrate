import { useState } from "react";
import { useApp } from "../App";
import { demoQuote } from "../demo";
import { Card, Field, useAsync } from "../ui";

type Token = { assetId: string; blockchain: string; symbol: string; decimals: number; price: number };
type Quote = { amountIn: string; amountInFormatted: string; amountOutFormatted: string; amountOutUsd?: string; timeEstimate: number; minAmountOut?: string };

const CHAIN: Record<string, string> = { eth: "Ethereum", arb: "Arbitrum", base: "Base", gnosis: "Gnosis", pol: "Polygon", avax: "Avalanche", bsc: "BNB Chain" };

export function PayPage() {
  const { demo } = useApp();
  const tokens = useAsync(async () => {
    if (demo) return [] as Token[];
    const r = await fetch("/api/quote");
    const b = await r.json();
    if (!r.ok) throw new Error(b.error || r.statusText);
    return b.tokens as Token[];
  }, [demo]);
  const [asset, setAsset] = useState("");
  const [amount, setAmount] = useState("100");
  const [quote, setQuote] = useState<Quote | null>(demo ? (demoQuote.quote as Quote) : null);
  const [recorded, setRecorded] = useState(demo);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const selected = tokens.data?.find((t) => t.assetId === (asset || tokens.data?.find((x) => x.blockchain === "base")?.assetId));

  const getQuote = async () => {
    if (!selected) return;
    setBusy(true);
    setErr(null);
    setQuote(null);
    setRecorded(false);
    try {
      const m = amount.trim().match(/^(\d+)(?:\.(\d+))?$/);
      if (!m) throw new Error("Enter an amount like 100 or 12.5");
      const frac = (m[2] || "").slice(0, selected.decimals).padEnd(selected.decimals, "0");
      const raw = (BigInt(m[1]) * 10n ** BigInt(selected.decimals) + BigInt(frac || "0")).toString();
      const r = await fetch("/api/quote", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ originAsset: selected.assetId, amount: raw }),
      });
      const b = await r.json();
      if (!r.ok || !b.quote) {
        const msg = b?.error?.message || (typeof b?.error === "string" ? b.error : JSON.stringify(b.error || b));
        throw new Error(`1Click did not return a quote: ${msg}`);
      }
      setQuote(b.quote);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Pay from any chain" right={<span className="pill pill-warn">quote only on testnet</span>}>
      <p className="small">
        On mainnet a buyer could pay in from another chain through NEAR Intents (1Click). 1Click has no testnet, so this panel asks
        1Click for a <strong>dry-run quote</strong> (<span className="mono">dry: true</span>) with today's mainnet prices: what it would
        cost to send USDC from an EVM chain and receive USDC on NEAR. Nothing is sent and no deposit address is created.
      </p>
      {demo ? (
        <p className="small muted">Scripted demo: showing the quote recorded on 2026-11-12 (fixtures/1click-quote.json).</p>
      ) : (
        <div className="form">
          <Field label="From">
            <select value={selected?.assetId || ""} onChange={(e) => setAsset(e.target.value)}>
              {tokens.data?.map((t) => (
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
            <button disabled={busy || !selected} onClick={getQuote}>
              {busy ? "Asking 1Click…" : "Get quote"}
            </button>
          </div>
        </div>
      )}
      {tokens.error && <p className="err">{tokens.error}</p>}
      {err && (
        <div>
          <p className="err small">{err}</p>
          <button
            className="ghost"
            onClick={() => {
              setQuote(demoQuote.quote as Quote);
              setRecorded(true);
              setErr(null);
            }}
          >
            Show the recorded quote instead
          </button>
        </div>
      )}
      {quote && recorded && (
        <p className="small">
          <span className="pill pill-warn">recorded</span> 10 USDC on Base → USDC on NEAR, quoted by 1Click on 2026-11-12 00:10 UTC
          (fixtures/1click-quote.json). Not a live price.
        </p>
      )}
      {quote && (
        <div className="grid4">
          <div className="stat">
            <small className="muted">You send</small>
            <div>{quote.amountInFormatted} USDC</div>
          </div>
          <div className="stat">
            <small className="muted">You receive on NEAR</small>
            <div>{quote.amountOutFormatted} USDC</div>
          </div>
          <div className="stat">
            <small className="muted">Cost</small>
            <div>{(Number(quote.amountInFormatted) - Number(quote.amountOutFormatted)).toFixed(4)} USDC</div>
          </div>
          <div className="stat">
            <small className="muted">Estimated time</small>
            <div>{quote.timeEstimate} s</div>
          </div>
        </div>
      )}
    </Card>
  );
}
