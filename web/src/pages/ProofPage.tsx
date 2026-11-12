import { href, useApp } from "../App";
import { EXPLORER_ACCOUNT, FLATRATE_ID, OUTLAYER_ID } from "../config";
import { getConfig, getMarkets } from "../data";
import { demoOracle } from "../demo";
import { listObservationTxs, readObservationTx, type ObservationTx } from "../proof";
import { near, usd, utc } from "../format";
import type { OracleOutput } from "../types";
import { Card, TxLink, useAsync } from "../ui";

export function ProofPage() {
  const { demo, step } = useApp();
  const cfg = useAsync(() => getConfig(demo), [demo]);
  const markets = useAsync(() => getMarkets(demo, step), [demo, step]);
  const txs = useAsync(
    async (): Promise<Map<string, ObservationTx>> => {
      const m = new Map<string, ObservationTx>();
      if (demo) {
        for (const [pid, v] of demoOracle(step))
          m.set(pid, { hash: v.hash, signer: "", pendingId: pid, recorded: true, rejectReason: null, oracle: v.oracle, chargedYocto: v.charged });
        return m;
      }
      const list = await listObservationTxs();
      const read = await Promise.all(list.map((t) => readObservationTx(t.hash, t.signer).catch(() => null)));
      for (const r of read) if (r?.pendingId) m.set(r.pendingId, r);
      return m;
    },
    [demo, step],
  );
  const rejected = txs.data ? [...txs.data.values()].filter((t) => !t.recorded) : [];

  return (
    <>
      <Card title="How prices get on-chain">
        <ol className="small">
          <li>
            Anyone calls <span className="mono">request_observation</span> on the contract during the period and pays OutLayer's fee.
          </li>
          <li>
            The contract calls <span className="mono">request_execution</span> on <span className="mono">{OUTLAYER_ID}</span> with the
            oracle project and the <strong>version pinned by wasm sha256</strong> when the market was created.
          </li>
          <li>
            The oracle fetches five public GPU price APIs inside the OutLayer worker and returns every source's price and the median.
          </li>
          <li>
            The result comes back by yield and resume to <span className="mono">on_observation</span>, which only the contract's own
            promise can reach, for a request id it issued and has not used. It needs at least 3 sources, a price above 0 and below 10 ×
            cap, and the stated median must equal the median of the sources. Otherwise nothing is recorded.
          </li>
          <li>After the period anyone can settle on the mean of the observations. There is no admin override.</li>
        </ol>
        {cfg.data && (
          <p className="small">
            Contract{" "}
            <a className="mono" href={EXPLORER_ACCOUNT + FLATRATE_ID} target="_blank" rel="noreferrer">
              {FLATRATE_ID}
            </a>{" "}
            · oracle project <span className="mono">{cfg.data.oracle_project_id}</span> · pinned version{" "}
            <span className="mono break">{cfg.data.oracle_version_key}</span> · sources {cfg.data.oracle_sources.join(", ")}
          </p>
        )}
        <p className="muted small">
          Trust note: OutLayer's own governance still runs on keys its operators hold (near-outlayer docs/NON_CUSTODIAL.md).
        </p>
      </Card>

      {txs.error && <Card><p className="err">Could not load transactions: {txs.error}</p></Card>}
      {markets.data
        ?.filter((m) => m.observations.length > 0)
        .map((m) => (
          <Card key={m.id} title={<a href={href(`/market/${m.id}`)}>Market {m.id}</a>} right={<small className="muted">{m.observations.length} observations</small>}>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Recorded</th>
                    <th>Median</th>
                    <th>OutLayer request</th>
                    <th>Tx</th>
                    <th>Per-source (USD/GPU-hr)</th>
                    <th>Fee</th>
                  </tr>
                </thead>
                <tbody>
                  {m.observations.map((o) => {
                    const t = txs.data?.get(o.pending_id);
                    const match = t?.oracle ? t.oracle.price_micros === Number(o.price_micros) : null;
                    return (
                      <tr key={o.pending_id}>
                        <td className="small">{utc(o.at)}</td>
                        <td>
                          {usd(o.price_micros, 3)}{" "}
                          {match === true && <span title="Stored price equals OutLayer's output in the transaction">✓</span>}
                          {match === false && <span className="err">mismatch</span>}
                        </td>
                        <td className="mono">{o.outlayer_request}</td>
                        <td>{t ? <TxLink hash={t.hash} /> : txs.loading ? "…" : "not found"}</td>
                        <td className="small">{t?.oracle ? <Sources o={t.oracle} /> : "—"}</td>
                        <td className="small">{t?.chargedYocto ? `${near(BigInt(t.chargedYocto), 5)} NEAR` : "—"}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </Card>
        ))}

      {rejected.length > 0 && (
        <Card title="Requests that recorded nothing">
          <ul className="small">
            {rejected.map((t) => (
              <li key={t.hash}>
                <TxLink hash={t.hash} /> · {t.rejectReason || "transaction failed before the callback"}
              </li>
            ))}
          </ul>
        </Card>
      )}
    </>
  );
}

function Sources({ o }: { o: OracleOutput }) {
  return (
    <span>
      {o.sources.map((s, i) => (
        <span key={s.name} title={s.error || ""}>
          {i > 0 && " · "}
          {s.name} {s.price_micros !== null ? (s.price_micros / 1e6).toFixed(3) : <span className="err">{s.error ? "error" : "—"}</span>}
        </span>
      ))}
    </span>
  );
}
