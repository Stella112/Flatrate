import { href, useApp } from "../App";
import { getMarkets } from "../data";
import { nextDeadline, statusLabel, timeLeft, usd, longPayout } from "../format";
import { Card, useAsync, useTick } from "../ui";

export function MarketsPage() {
  const { demo, step } = useApp();
  useTick(1000);
  const { data, error, loading } = useAsync(() => getMarkets(demo, step), [demo, step], demo ? undefined : 20000);

  return (
    <>
      <Card title="Lock next week's GPU price">
        <p>
          Each market tracks the H100 price per GPU-hour. A <strong>Long</strong> claim pays more when the price rises (for buyers of
          compute), a <strong>Short</strong> claim pays more when it falls (for sellers). One fUSD backs one Long plus one Short. At
          settlement Long pays <span className="mono">clamp((avg − floor) / (cap − floor), 0, 1)</span> fUSD and Short pays the rest.
          Prices come from five public GPU price APIs, fetched inside an OutLayer enclave; the median is posted on-chain.
        </p>
        <p className="muted">Testnet only. fUSD is a test token with a public faucet.</p>
      </Card>
      <Card title="Markets">
        {loading && !data && <p className="muted">Loading…</p>}
        {error && <p className="err">{error}</p>}
        {data && (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>#</th>
                  <th>Status</th>
                  <th>Floor</th>
                  <th>Cap</th>
                  <th>Latest median</th>
                  <th>Obs.</th>
                  <th>Next</th>
                </tr>
              </thead>
              <tbody>
                {data.map((m) => {
                  const last = m.observations[m.observations.length - 1];
                  const nd = nextDeadline(m);
                  const lp = longPayout(m.status);
                  return (
                    <tr key={m.id} onClick={() => (window.location.href = href(`/market/${m.id}`))} className="click">
                      <td>
                        <a href={href(`/market/${m.id}`)}>{m.id}</a>
                      </td>
                      <td>
                        <span className={`pill pill-${statusLabel(m.status).toLowerCase()}`}>{statusLabel(m.status)}</span>
                        {lp !== null && <small className="muted"> Long {usd(lp, 4)}</small>}
                      </td>
                      <td>{usd(m.floor_micros)}</td>
                      <td>{usd(m.cap_micros)}</td>
                      <td>{last ? usd(last.price_micros, 3) : "—"}</td>
                      <td>
                        {m.observations.length}/{m.min_observations}
                      </td>
                      <td>{nd && !demo ? `${nd.label} in ${timeLeft(nd.ns)}` : "—"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </>
  );
}
