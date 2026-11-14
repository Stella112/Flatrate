import { INDEX } from "../config";
import { href, useApp } from "../App";
import { getMarkets } from "../data";
import { longPayout, nextDeadline, phase, statusLabel, timeLeft, units, usd } from "../format";
import type { Market } from "../types";
import { useAsync, useTick } from "../ui";

const PHASE_TEXT: Record<ReturnType<typeof phase>, string> = {
  commit: INDEX.privateAuction ? "Sealed bids open: encrypt and commit" : "Sealed bids open: commit a hash",
  reveal: "Reveal window: bidders open their bids",
  clear: INDEX.privateAuction ? "Ready to clear in the enclave" : "Ready to clear: anyone can press Clear",
  period: "Recording one attested price per slot",
  settle: "Period over: anyone can settle",
  redeem: "Settled: holders redeem",
};

/** Where the latest price sits inside the band, 0..100 (clamped). */
function bandPos(m: Market): number | null {
  const last = m.observations[m.observations.length - 1];
  const p = last ? Number(last.price_micros) : m.settled_average_micros ? Number(m.settled_average_micros) : null;
  if (p === null) return null;
  const f = Number(m.floor_micros);
  const c = Number(m.cap_micros);
  return Math.max(0, Math.min(100, ((p - f) / (c - f)) * 100));
}

function MarketCard({ m, demo }: { m: Market; demo: boolean }) {
  const last = m.observations[m.observations.length - 1];
  const nd = nextDeadline(m);
  const lp = longPayout(m.status);
  const pos = bandPos(m);
  const label = statusLabel(m.status);
  const ph = phase(m);
  return (
    <a className="mcard" href={href(`/market/${m.id}`)}>
      <div className="mcard-top">
        <div className="mcard-id">
          Market {m.id}
          <small>{m.template === "DEMO" ? "Demo · minutes" : "Weekly"}</small>
        </div>
        <span className={`pill pill-${label.toLowerCase()}`}>{label}</span>
      </div>
      <div>
        <div className="band" title="Band: below the floor Short gets everything, above the cap Long does">
          {pos !== null && <div className="band-marker" style={{ left: `${pos}%` }} />}
        </div>
        <div className="band-labels">
          <span>floor {usd(m.floor_micros)}</span>
          <span>cap {usd(m.cap_micros)}</span>
        </div>
      </div>
      <div className="mcard-stats">
        <div className="mstat">
          <span>{m.settled_average_micros ? "Settled average" : "Latest median"}</span>
          <b>{m.settled_average_micros ? usd(m.settled_average_micros, 3) : last ? usd(last.price_micros, 3) : "—"}</b>
        </div>
        <div className="mstat">
          <span>{lp !== null ? "Long pays" : "Opening price"}</span>
          <b>{lp !== null ? usd(lp, 4) : m.opening_price_cents ? `${m.opening_price_cents}¢ Long` : "—"}</b>
        </div>
        <div className="mstat">
          <span>Collateral</span>
          <b>{units(m.collateral)} fUSD</b>
        </div>
      </div>
      <div>
        <div className="slots" title={`${m.filled_slots} of ${m.num_slots} price slots filled, ${m.min_filled_slots} needed`}>
          {Array.from({ length: m.num_slots }, (_, i) => (
            <div key={i} className={`slot ${m.observations.some((o) => o.slot === i) ? "filled" : ""}`} />
          ))}
        </div>
        <div className="mcard-foot" style={{ marginTop: 6 }}>
          <span>
            {m.filled_slots}/{m.num_slots} prices · {m.min_filled_slots} needed
          </span>
          <span>{nd && !demo ? `${nd.label} in ${timeLeft(nd.ns)}` : ""}</span>
        </div>
      </div>
      <div className="mcard-foot">
        <span style={{ color: "var(--text-2)" }}>{PHASE_TEXT[ph]}</span>
        <span>Open →</span>
      </div>
    </a>
  );
}

export function MarketsPage() {
  const { demo, step } = useApp();
  useTick(1000);
  const { data, error, loading } = useAsync(() => getMarkets(demo, step), [demo, step], demo ? undefined : 20000);
  const live = (data ?? []).filter((m) => m.status === "Auction" || m.status === "Open");
  const done = (data ?? []).filter((m) => !(m.status === "Auction" || m.status === "Open"));
  const latest = (data ?? []).flatMap((m) => m.observations.map((o) => ({ o, m }))).sort((a, b) => Number(BigInt(b.o.at) - BigInt(a.o.at)))[0];
  const collateral = (data ?? []).reduce((n, m) => n + BigInt(m.collateral), 0n);

  return (
    <>
      <div className="kpis">
        <div className="kpi">
          <div className="kpi-label">Live markets</div>
          <div className="kpi-value">{data ? live.length : "…"}</div>
          <div className="kpi-sub">{data ? `${done.length} settled or void` : ""}</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">Latest attested median</div>
          <div className="kpi-value">{latest ? usd(latest.o.price_micros, INDEX.key === "tokens" ? 2 : 2) : "—"}</div>
          <div className="kpi-sub">{latest ? `market ${latest.m.id}, slot ${latest.o.slot}, ${latest.o.sources_ok} sources` : "no prices yet"}</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">Collateral held</div>
          <div className="kpi-value">{data ? `${units(collateral)}` : "…"}</div>
          <div className="kpi-sub">fUSD held for claims not yet redeemed</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">Who can run it</div>
          <div className="kpi-value">Anyone</div>
          <div className="kpi-sub">roll, record prices, settle: public calls</div>
        </div>
      </div>

      {!demo && (
        <div className="callout">
          <span>💡</span>
          <span>
            New here? Create a guest account in the sidebar (no wallet), open a <b>Demo</b> market, and follow its steps: it
            runs from sealed bids to payout in about 15 minutes. If the latest demo is settled, open it and press <b>Roll the next market</b>.
          </span>
        </div>
      )}

      <div className="lifecycle">
        <div className="lc-step">
          <div className="lc-n">1 · Auction</div>
          <div className="lc-t">{INDEX.privateAuction ? "Encrypted bids" : "Sealed bids"}</div>
          <div className="lc-d">{INDEX.privateAuction ? "Cleared inside an OutLayer enclave" : "Commit, reveal, one clearing price"}</div>
        </div>
        <div className="lc-step">
          <div className="lc-n">2 · Open</div>
          <div className="lc-t">Claims trade</div>
          <div className="lc-d">Mint pairs at $1, transfer, wrap in a Cover Note</div>
        </div>
        <div className="lc-step">
          <div className="lc-n">3 · Prices</div>
          <div className="lc-t">One per slot</div>
          <div className="lc-d">Median of several APIs, fetched in a TEE</div>
        </div>
        <div className="lc-step">
          <div className="lc-n">4 · Settle</div>
          <div className="lc-t">Period average</div>
          <div className="lc-d">Too few prices: midpoint after the void time</div>
        </div>
        <div className="lc-step">
          <div className="lc-n">5 · Redeem</div>
          <div className="lc-t">Paid in fUSD</div>
          <div className="lc-d">Long + Short always equal $1</div>
        </div>
      </div>

      {loading && !data && <p className="muted">Loading markets…</p>}
      {error && <p className="err">{error}</p>}
      {data && live.length > 0 && (
        <>
          <div className="section-h">
            <h2>Live</h2>
            <small className="muted">auctions and open periods</small>
          </div>
          <div className="mgrid" style={{ marginBottom: 28 }}>
            {live.map((m) => (
              <MarketCard key={m.id} m={m} demo={demo} />
            ))}
          </div>
        </>
      )}
      {data && done.length > 0 && (
        <>
          <div className="section-h">
            <h2>Settled</h2>
            <small className="muted">every transaction is on the Proof page and the explorer</small>
          </div>
          <div className="mgrid">
            {done.map((m) => (
              <MarketCard key={m.id} m={m} demo={demo} />
            ))}
          </div>
        </>
      )}
    </>
  );
}
