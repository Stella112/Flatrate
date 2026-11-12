import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { EXPLORER_TX } from "./config";
import type { TxResult } from "./near";
import { shortHash } from "./format";

export function useAsync<T>(fn: () => Promise<T>, deps: unknown[], refreshMs?: number) {
  const [data, setData] = useState<T | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [loading, setLoading] = useState(true);
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const reload = useCallback(async () => {
    try {
      setData(await fnRef.current());
      setError(undefined);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    setLoading(true);
    reload();
    if (!refreshMs) return;
    const t = setInterval(reload, refreshMs);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return { data, error, loading, reload };
}

/** Re-renders every `ms` so countdowns move. */
export function useTick(ms = 1000) {
  const [, set] = useState(0);
  useEffect(() => {
    const t = setInterval(() => set((x) => x + 1), ms);
    return () => clearInterval(t);
  }, [ms]);
}

export const TxLink = ({ hash }: { hash: string }) =>
  hash ? (
    <a href={EXPLORER_TX + hash} target="_blank" rel="noreferrer" className="mono">
      {shortHash(hash)}
    </a>
  ) : null;

export function TxView({ r }: { r: TxResult | null }) {
  if (!r) return null;
  const failed = !r.ok || !!r.error;
  return (
    <div className={`tx ${failed ? "tx-fail" : "tx-ok"}`}>
      <div>
        {failed ? "Failed" : "Done"} · tx <TxLink hash={r.hash} />
        {r.error && <span className="mono"> · {r.error.length > 160 ? r.error.slice(0, 160) + "…" : r.error}</span>}
      </div>
      {r.events.length > 0 && (
        <ul className="events">
          {r.events
            .filter((e) => !["ft_transfer"].includes(e.event))
            .map((e, i) => (
              <li key={i} className="mono">
                {e.event} {JSON.stringify(e.data)}
              </li>
            ))}
        </ul>
      )}
    </div>
  );
}

/** Runs one transaction-producing action and shows its result. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<TxResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (fn: () => Promise<TxResult>, after?: () => void) => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const r = await fn();
      setResult(r);
      after?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, []);
  const view = (
    <>
      <TxView r={result} />
      {error && <div className="tx tx-fail">{error}</div>}
    </>
  );
  return { busy, run, view, result };
}

export const Card = ({ title, children, right }: { title?: ReactNode; children: ReactNode; right?: ReactNode }) => (
  <section className="card">
    {(title || right) && (
      <header className="card-h">
        <h2>{title}</h2>
        {right}
      </header>
    )}
    {children}
  </section>
);

export const Field = ({ label, children, hint }: { label: string; children: ReactNode; hint?: ReactNode }) => (
  <label className="field">
    <span>{label}</span>
    {children}
    {hint && <small>{hint}</small>}
  </label>
);
