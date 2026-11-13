import { INDEX } from "./config";
import type { Observation } from "./types";
import { usd, utc } from "./format";

/** Observed medians against the market's floor and cap. */
export function PriceChart({ obs, floor, cap }: { obs: Observation[]; floor: number; cap: number }) {
  const W = 640;
  const H = 220;
  const pad = { l: 56, r: 16, t: 14, b: 28 };
  const prices = obs.map((o) => Number(o.price_micros));
  const lo = Math.min(floor, ...prices) * 0.95;
  const hi = Math.max(cap, ...prices) * 1.05;
  const x = (i: number) => pad.l + (obs.length <= 1 ? (W - pad.l - pad.r) / 2 : (i * (W - pad.l - pad.r)) / (obs.length - 1));
  const y = (v: number) => pad.t + ((hi - v) * (H - pad.t - pad.b)) / (hi - lo);
  const path = prices.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p).toFixed(1)}`).join(" ");
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img" aria-label={`Observed price, ${INDEX.unit}`}>
      {[["cap", cap], ["floor", floor]].map(([name, v]) => (
        <g key={name as string}>
          <line x1={pad.l} x2={W - pad.r} y1={y(v as number)} y2={y(v as number)} className="chart-bound" />
          <text x={pad.l - 6} y={y(v as number) + 4} textAnchor="end" className="chart-label">
            {usd(v as number)}
          </text>
          <text x={W - pad.r} y={y(v as number) - 5} textAnchor="end" className="chart-label">
            {name}
          </text>
        </g>
      ))}
      {obs.length === 0 && (
        <text x={W / 2} y={H / 2} textAnchor="middle" className="chart-label">
          No observations yet
        </text>
      )}
      {obs.length > 1 && <path d={path} className="chart-line" />}
      {obs.map((o, i) => (
        <g key={o.pending_id}>
          <circle cx={x(i)} cy={y(prices[i])} r={4} className="chart-dot">
            <title>{`${usd(prices[i], 3)} at ${utc(o.at)}, ${o.sources_ok} sources, OutLayer request ${o.outlayer_request}`}</title>
          </circle>
        </g>
      ))}
      {obs.length > 0 && (
        <text x={x(obs.length - 1)} y={y(prices[prices.length - 1]) - 9} textAnchor="middle" className="chart-value">
          {usd(prices[prices.length - 1], 3)}
        </text>
      )}
    </svg>
  );
}
