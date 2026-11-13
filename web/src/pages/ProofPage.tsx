import { href, useApp } from "../App";
import { EXPLORER_ACCOUNT, FLATRATE_ID, OUTLAYER_ID } from "../config";
import { getConfig, getMarkets } from "../data";
import { demoOracle } from "../demo";
import { checkAttestation, listObservationTxs, OUTLAYER_API, readObservationTx, type AttestationCheck, type ObservationTx } from "../proof";
import { near, usd, utc } from "../format";
import type { OracleOutput } from "../types";
import { Card, TxLink, useAsync } from "../ui";
import { AttackPanel } from "../Attacks";

export function ProofPage() {
  const { demo, step } = useApp();
  const cfg = useAsync(() => getConfig(demo), [demo]);
  const markets = useAsync(() => getMarkets(demo, step), [demo, step]);
  const txs = useAsync(
    async (): Promise<Map<string, ObservationTx>> => {
      const m = new Map<string, ObservationTx>();
      if (demo) {
        for (const [pid, v] of demoOracle(step))
          m.set(pid, { hash: v.hash, signer: "", pendingId: pid, recorded: true, rejectReason: null, oracle: v.oracle, oracleRaw: null, chargedYocto: v.charged });
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
  const atts = useAsync(
    async (): Promise<Map<string, AttestationCheck | null>> => {
      const out = new Map<string, AttestationCheck | null>();
      if (demo || !markets.data || !txs.data) return out;
      for (const m of markets.data) {
        for (const o of m.observations) {
          if (!o.outlayer_request) continue;
          const t = txs.data.get(o.pending_id);
          out.set(o.pending_id, await checkAttestation(o.outlayer_request, m.oracle_version_key, m.oracle_input, t?.oracleRaw ?? null).catch(() => null));
        }
      }
      return out;
    },
    [markets.data, txs.data],
  );

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
          <li>
            One price per time slot. After the period anyone can settle on the mean of the filled slots. There is no admin
            override. Anyone can then roll the next market; its band is centred on the settled average.
          </li>
        </ol>
        <p className="small">
          <strong>Attestation.</strong> For every price the table fetches OutLayer's attestation record (TDX quote, worker
          measurement, hashes) and checks that its wasm hash is the market's pinned version, its input hash is the sha256 of
          the market's oracle input, and its output hash is the sha256 of the exact bytes the contract received. The contract
          itself trusts the OutLayer contract and the pinned version; it does not verify the TDX quote.
        </p>
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

      <AttackPanel />

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
                    <th>Attestation</th>
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
                        <td className="small">
                          <AttCell c={atts.data?.get(o.pending_id)} loading={atts.loading} requestId={o.outlayer_request} />
                        </td>
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

function AttCell({ c, loading, requestId }: { c: AttestationCheck | null | undefined; loading: boolean; requestId: string }) {
  if (c === undefined) return <>{loading ? "…" : "—"}</>;
  if (c === null) return <span className="muted">not found</span>;
  const mark = (ok: boolean | null) => (ok === null ? "?" : ok ? "✓" : "✗");
  return (
    <a
      href={`${OUTLAYER_API}/attestations/by-request/${requestId}`}
      target="_blank"
      rel="noreferrer"
      title={`worker ${c.att.worker_measurement.slice(0, 16)}…, TDX quote ${c.att.tdx_quote.length} chars`}
    >
      wasm {mark(c.wasmMatches)} · input {mark(c.inputMatches)} · output {mark(c.outputMatches)}
    </a>
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
