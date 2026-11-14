// Landing page: the story of Flatrate, separate from the app (#/app). Every number here is either
// read live from testnet or carries its source; every feature line has a row in CLAIMS.md.
import { useMemo, useState, type JSX } from "react";
import { INDEXES, RPC_URL } from "../config";
import { view } from "../near";
import { usd } from "../format";
import type { Market } from "../types";
import { useAsync } from "../ui";
import { Logo } from "../Logo";

const REPO = "https://github.com/Stella112/Flatrate";

type Live = {
  markets: number;
  settled: number;
  prices: number;
  latestH100: { price: string; market: string } | null;
  latestTokens: string | null;
  keyless: boolean;
  h100Latest: Market | null;
};

async function readLive(): Promise<Live> {
  const lists = await Promise.all(
    INDEXES.map((i) => view<Market[]>(i.contractId, "list_markets", { limit: 100 }).catch(() => [] as Market[])),
  );
  const all = lists.flat();
  const settled = all.filter((m) => typeof m.status === "object" || m.status === "Void").length;
  const prices = all.reduce((n, m) => n + m.observations.length, 0);
  const latestObs = (ms: Market[]) => {
    let best: { price: string; at: bigint; market: string } | null = null;
    for (const m of ms)
      for (const o of m.observations) if (!best || BigInt(o.at) > best.at) best = { price: o.price_micros, at: BigInt(o.at), market: m.id };
    return best;
  };
  const h = latestObs(lists[0]);
  const t = latestObs(lists[1] || []);
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: "lp",
      method: "query",
      params: { request_type: "view_access_key_list", finality: "final", account_id: INDEXES[0].contractId },
    }),
  })
    .then((r) => r.json())
    .catch(() => null);
  const keyless = !!res?.result && res.result.keys.length === 0;
  const h100Latest = lists[0].slice().sort((a, b) => Number(BigInt(b.id) - BigInt(a.id)))[0] ?? null;
  return {
    markets: all.length,
    settled,
    prices,
    latestH100: h ? { price: h.price, market: h.market } : null,
    latestTokens: t ? t.price : null,
    keyless,
    h100Latest,
  };
}

const I = {
  lock: <path d="M5 10V7a5 5 0 0 1 10 0v3M4 10h12v8H4z" />,
  shield: <path d="M10 2l7 3v5c0 4.5-3 7.5-7 8-4-.5-7-3.5-7-8V5z" />,
  chip: <path d="M6 6h8v8H6zM8 2v4M12 2v4M8 14v4M12 14v4M2 8h4M2 12h4M14 8h4M14 12h4" />,
  note: <path d="M4 3h9l3 3v11H4zM7 9h6M7 12h6M7 6h3" />,
  bot: <path d="M5 8h10v8H5zM10 4v4M7.5 12h.01M12.5 12h.01M2 11v3M18 11v3" />,
  swap: <path d="M4 7h11l-3-3M16 13H5l3 3" />,
  eye: <path d="M2 10s3-6 8-6 8 6 8 6-3 6-8 6-8-6-8-6zM10 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z" />,
  coins: <path d="M7 6a4 2 0 1 0 8 0 4 2 0 1 0-8 0M7 6v4c0 1.1 1.8 2 4 2s4-.9 4-2V6M3 10a4 2 0 1 0 8 0M3 10v4c0 1.1 1.8 2 4 2s4-.9 4-2" />,
  link: <path d="M8 12l4-4M7 9l-2 2a3 3 0 0 0 4 4l2-2M13 11l2-2a3 3 0 0 0-4-4l-2 2" />,
};
const Icon = ({ d }: { d: JSX.Element }) => (
  <span className="icon">
    <svg viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      {d}
    </svg>
  </span>
);

/** Long payout for the description's example band, as a function of the settled average. */
function PayoffCalc() {
  const floor = 2.5;
  const cap = 4.5;
  const [avg, setAvg] = useState(3.5);
  const long = Math.min(1, Math.max(0, (avg - floor) / (cap - floor)));
  return (
    <div className="calc">
      <div>
        <div className="sec-k">Try the payoff</div>
        <h3 style={{ fontSize: "1.3em", margin: "8px 0 6px" }}>Band $2.50 – $4.50 per H100 hour</h3>
        <p className="muted" style={{ margin: "0 0 18px" }}>
          Drag the settled average. One Long plus one Short always share exactly one dollar of collateral, so nothing can be
          liquidated and a buyer can lose at most what it paid.
        </p>
        <input type="range" min={1.5} max={5.5} step={0.01} value={avg} onChange={(e) => setAvg(Number(e.target.value))} aria-label="Settled average price" />
        <div className="band-labels">
          <span>$1.50</span>
          <span style={{ color: "var(--text)" }}>average ${avg.toFixed(2)}/hr</span>
          <span>$5.50</span>
        </div>
        <div className="formula">long = clamp((average − floor) / (cap − floor), 0, 1) · short = 1 − long</div>
      </div>
      <div className="calc-out">
        <div className="payout long">
          <span>Each Long claim pays</span>
          <b>${long.toFixed(2)}</b>
          <span>for the team that rents GPUs</span>
        </div>
        <div className="payout short">
          <span>Each Short claim pays</span>
          <b>${(1 - long).toFixed(2)}</b>
          <span>for the GPU owner</span>
        </div>
      </div>
    </div>
  );
}

/** A small live preview of the app for the hero frame. */
function Preview({ live }: { live: Live | undefined }) {
  const m = live?.h100Latest;
  const obs = m?.observations ?? [];
  const pts = useMemo(() => {
    if (!m || obs.length === 0) return "";
    const f = Number(m.floor_micros);
    const c = Number(m.cap_micros);
    return obs
      .map((o, i) => {
        const x = obs.length === 1 ? 50 : (i / (obs.length - 1)) * 100;
        const y = 100 - ((Number(o.price_micros) - f) / (c - f)) * 100;
        return `${x.toFixed(1)},${Math.max(0, Math.min(100, y)).toFixed(1)}`;
      })
      .join(" ");
  }, [m, obs]);
  const status = !m ? "…" : typeof m.status === "object" ? "Settled" : m.status;
  return (
    <div className="frame" aria-label="Live preview of the Flatrate app">
      <div className="frame-bar">
        <div className="frame-dots">
          <i />
          <i />
          <i />
        </div>
        <span>flatrate-gpu.vercel.app/#/app</span>
        <span style={{ marginLeft: "auto" }}>
          <span className="dot-live" />
          live testnet data
        </span>
      </div>
      <div className="frame-body">
        <div className="frame-side">
          <span className="on">Markets</span>
          <span>My notes</span>
          <span>Proof</span>
          <span>Pay from any chain</span>
          <span style={{ marginTop: 10, fontSize: "0.85em", color: "var(--faint)" }}>INDEXES</span>
          <span className="on">H100 GPU-hour</span>
          <span>LLM tokens</span>
          <span>H100 private auction</span>
        </div>
        <div className="frame-main">
          <div className="mcard-top">
            <div className="mcard-id">
              {m ? `Market ${m.id}` : "Market"}
              <small>{m?.template ?? ""} · H100 per GPU-hour</small>
            </div>
            <span className={`pill pill-${status.toLowerCase()}`}>{status}</span>
          </div>
          <div className="frame-row">
            <div className="kpi" style={{ border: "1px solid var(--line)", borderRadius: 10 }}>
              <div className="kpi-label">Latest attested median</div>
              <div className="kpi-value">{live?.latestH100 ? usd(live.latestH100.price, 2) : "—"}</div>
              <div className="kpi-sub">from 5 GPU price APIs, via OutLayer</div>
            </div>
            <div className="kpi" style={{ border: "1px solid var(--line)", borderRadius: 10 }}>
              <div className="kpi-label">Band</div>
              <div className="kpi-value">{m ? `${usd(m.floor_micros)} – ${usd(m.cap_micros)}` : "—"}</div>
              <div className="kpi-sub">set from the last settled average</div>
            </div>
          </div>
          <svg viewBox="0 0 100 100" preserveAspectRatio="none" style={{ width: "100%", height: 120, border: "1px solid var(--line)", borderRadius: 10, background: "var(--bg-2)" }}>
            <line x1="0" y1="0" x2="100" y2="0" stroke="var(--chart-bound)" strokeDasharray="2 2" vectorEffect="non-scaling-stroke" />
            <line x1="0" y1="100" x2="100" y2="100" stroke="var(--chart-bound)" strokeDasharray="2 2" vectorEffect="non-scaling-stroke" />
            {pts && <polyline points={pts} fill="none" stroke="var(--accent)" strokeWidth="2" vectorEffect="non-scaling-stroke" />}
          </svg>
          <div className="slots">
            {Array.from({ length: m?.num_slots ?? 8 }, (_, i) => (
              <div key={i} className={`slot ${obs.some((o) => o.slot === i) ? "filled" : ""}`} />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export function Landing() {
  const { data: live } = useAsync(readLive, [], 30000);
  return (
    <div className="lp">
      <nav className="lp-nav">
        <div className="lp-nav-in">
          <a href="#/" className="side-brand" style={{ padding: 0 }}>
            <Logo /> Flatrate
          </a>
          <div className="lp-links">
            <a href="#problem">Problem</a>
            <a href="#how">How it works</a>
            <a href="#built">What's built</a>
            <a href="#trust">Trust</a>
            <a href={REPO} target="_blank" rel="noreferrer">
              GitHub
            </a>
          </div>
          <a className="btn btn-sm" href="#/app">
            Open the app
          </a>
        </div>
      </nav>

      <header className="wrap hero">
        <a className="eyebrow" href="#built">
          <b>Testnet</b> Built at the NEAR Hackathon →
        </a>
        <h1>Lock next week's AI compute bill.</h1>
        <p className="hero-sub">
          <b>Fully backed, never liquidated, priced by a feed no operator can set.</b> Flatrate lets AI teams and agents fix the
          price of an H100 hour on NEAR with one-dollar claims.
        </p>
        <div className="hero-cta">
          <a className="btn" href="#/app">
            Open the app →
          </a>
          <a className="btn btn-ghost" href="?demo=scripted#/app">
            Watch the scripted demo
          </a>
          <a className="btn btn-ghost" href="#/proof">
            See the proof
          </a>
        </div>
        <div className="hero-meta">No wallet needed · guest account in the browser · NEAR testnet</div>
        <Preview live={live} />
      </header>

      <div className="wrap">
        <div className="logos" aria-label="Built on">
          <span>NEAR</span>
          <span>OutLayer TEE</span>
          <span>NEAR Intents</span>
          <span>Aurora Intents</span>
          <span>Chain Signatures</span>
          <span>Rhea</span>
        </div>
        <div className="logos-cap">The infrastructure Flatrate runs on</div>

        <p className="statement">
          A price lock for GPU time. <span>AI builders sell at fixed prices but buy compute at floating ones. When the price of an H100 hour jumps, the margin disappears. Flatrate turns that risk into a small, fully backed claim anyone can hold.</span>
        </p>

        <section className="sec" id="problem">
          <div className="sec-k">The problem</div>
          <h2>The price of compute really moves.</h2>
          <p className="sec-lead">Teams keep renting wherever they like. What they cannot control is what an hour costs next week.</p>
          <div className="cols3">
            <div className="tile">
              <div className="tile-big">$7–10 → $2–4</div>
              <h3>H100 per hour, early 2024 to late 2025</h3>
              <p>Rental prices fell by more than half, and they keep moving week to week.</p>
              <a className="src" href="https://www.silicondata.com/blog/h100-rental-price-over-time" target="_blank" rel="noreferrer">
                Source: Silicon Data
              </a>
            </div>
            <div className="tile">
              <div className="tile-big">$2.69 vs $4.17</div>
              <h3>The same H100 hour, the same day</h3>
              <p>RunPod against Shadeform's clouds on 13 October 2026. The spread alone is a margin.</p>
              <a className="src" href="https://github.com/Cassxbt/piry" target="_blank" rel="noreferrer">
                Source: PIRY README
              </a>
            </div>
            <div className="tile">
              <div className="tile-big">730 hours</div>
              <h3>One CME compute future</h3>
              <p>Regulated contracts are sized for funds. Crypto versions are leveraged perpetuals. None fits one agent's bill.</p>
              <a className="src" href="https://www.fia.org/fia/articles/explainer-what-are-compute-futures-and-how-do-they-work" target="_blank" rel="noreferrer">
                Source: FIA explainer
              </a>
            </div>
          </div>
        </section>

        <section className="sec" id="how">
          <div className="sec-k">How it works</div>
          <h2>One market, six steps, no operator.</h2>
          <p className="sec-lead">Every market for an index (H100 per GPU-hour, or LLM tokens) moves through the same lifecycle. Anyone can push it forward.</p>
          <div className="steps">
            <div className="step">
              <div className="step-n">01 · Mint</div>
              <h3>One dollar, two claims</h3>
              <p>1 fUSD mints one Long and one Short. Long pays more if the price rises, Short if it falls.</p>
            </div>
            <div className="step">
              <div className="step-n">02 · Sealed auction</div>
              <h3>One opening price</h3>
              <p>Bids are committed as hashes, revealed, and cleared at a single price. Unused escrow is refunded.</p>
            </div>
            <div className="step">
              <div className="step-n">03 · Hold or hand on</div>
              <h3>Transfer, wrap, trade</h3>
              <p>Move claims, wrap them in a Cover Note NFT, or trade them as a token in a Rhea pool.</p>
            </div>
            <div className="step">
              <div className="step-n">04 · Attested prices</div>
              <h3>One price per time slot</h3>
              <p>OutLayer fetches several public price APIs inside a TEE and posts the median. One price per slot, so nobody can skew the average.</p>
            </div>
            <div className="step">
              <div className="step-n">05 · Settle</div>
              <h3>On the period average</h3>
              <p>After the period anyone settles. Too few prices and the market settles at the band's midpoint.</p>
            </div>
            <div className="step">
              <div className="step-n">06 · Redeem and roll</div>
              <h3>Paid, then the next week</h3>
              <p>Holders redeem. Anyone opens the next market, with its band set from the last average.</p>
            </div>
          </div>
          <PayoffCalc />
        </section>

        <section className="sec" id="live">
          <div className="sec-k">Live on testnet</div>
          <h2>Not a mock-up. Read from the chain right now.</h2>
          <p className="sec-lead">These numbers come from the deployed contracts as this page loads.</p>
          <div className="live">
            <div>
              <b>{live ? live.markets : "…"}</b>
              <span>markets across 3 contracts</span>
            </div>
            <div>
              <b>{live ? live.settled : "…"}</b>
              <span>settled or voided on-chain</span>
            </div>
            <div>
              <b>{live ? live.prices : "…"}</b>
              <span>attested prices recorded</span>
            </div>
            <div>
              <b>{live?.latestH100 ? usd(live.latestH100.price, 2) : "…"}</b>
              <span>
                latest H100 median per hour{live?.latestTokens ? ` · LLM tokens ${usd(live.latestTokens, 2)}/1M` : ""}
              </span>
            </div>
          </div>
        </section>

        <section className="sec" id="built">
          <div className="sec-k">What's built</div>
          <h2>Everything below runs on NEAR testnet.</h2>
          <p className="sec-lead">Each line has a test or a testnet transaction in the repository's claims table.</p>
          <div className="cols3">
            <div className="tile">
              <Icon d={I.lock} />
              <h3>Fully backed claims</h3>
              <p>Long and Short share one dollar. No leverage, no margin, no liquidation. Mint, redeem as a pair, transfer.</p>
            </div>
            <div className="tile">
              <Icon d={I.chip} />
              <h3>TEE price feed</h3>
              <p>A pinned, reproducible oracle runs inside OutLayer and returns each source's price and the median, with an attestation per price.</p>
            </div>
            <div className="tile">
              <Icon d={I.note} />
              <h3>Cover Note NFTs</h3>
              <p>A NEP-171 token that holds a hedge and pays whoever owns it at settlement. Sell it, lend against it, unwrap it.</p>
            </div>
            <div className="tile">
              <Icon d={I.eye} />
              <h3>Sealed and private auctions</h3>
              <p>Commit-reveal openings, and a private version where bids are encrypted to an OutLayer enclave and cleared inside it.</p>
            </div>
            <div className="tile">
              <Icon d={I.bot} />
              <h3>Agents that hedge</h3>
              <p>A script reads a usage forecast, sizes a Long hedge, bids within a spending cap and runs the market to redemption.</p>
            </div>
            <div className="tile">
              <Icon d={I.coins} />
              <h3>LLM token prices</h3>
              <p>The same design on a second contract, priced on Llama 3.3 70B output tokens across seven providers.</p>
            </div>
            <div className="tile">
              <Icon d={I.swap} />
              <h3>Trade on Rhea</h3>
              <p>Claims wrap into a NEP-141 token and trade against fUSD in a Rhea pool, then unwrap back into claims.</p>
            </div>
            <div className="tile">
              <Icon d={I.link} />
              <h3>Pay from any chain</h3>
              <p>Live NEAR Intents and Aurora Intents quotes side by side, and a NEAR account signing on Base Sepolia through chain signatures.</p>
            </div>
            <div className="tile">
              <Icon d={I.shield} />
              <h3>Keyless contract</h3>
              <p>The H100 contract has no access keys{live ? (live.keyless ? " (checked live: 0 keys)" : "") : ""}. Anyone opens the next market, records prices and settles.</p>
            </div>
          </div>
        </section>

        <section className="sec" id="trust">
          <div className="sec-k">Trust</div>
          <h2>Try to break it. Each attack fails on-chain.</h2>
          <p className="sec-lead">The Proof page has a button for each of these. They send real testnet transactions and show the contract's refusal.</p>
          <div className="attack-list">
            <div>
              <span>Post a fake price from any account that is not OutLayer's callback</span>
              <code>ERR_NOT_OUTLAYER</code>
            </div>
            <div>
              <span>Record a second price in a time slot that already has one</span>
              <code>ERR_SLOT_FILLED</code>
            </div>
            <div>
              <span>Settle a market before its period ends</span>
              <code>ERR_PERIOD_NOT_ENDED</code>
            </div>
            <div>
              <span>Redeem a Cover Note you do not own</span>
              <code>ERR_NOT_OWNER</code>
            </div>
          </div>
          <div className="row" style={{ marginTop: 18 }}>
            <a className="btn btn-ghost btn-sm" href="#/proof">
              Open the Proof page →
            </a>
            <a className="btn btn-ghost btn-sm" href={`${REPO}/blob/main/CLAIMS.md`} target="_blank" rel="noreferrer">
              Read the claims table →
            </a>
          </div>
        </section>

        <section className="sec" id="limits">
          <div className="sec-k">Limitations</div>
          <h2>What it does not do, stated plainly.</h2>
          <ul className="limits" style={{ marginTop: 24 }}>
            <li>
              <b>Testnet only.</b> Collateral is a test stablecoin with a public faucet. No real money is at risk.
            </li>
            <li>
              <b>Both sides seeded by us.</b> There are no outside traders yet; a hedging market needs both.
            </li>
            <li>
              <b>Trust in OutLayer.</b> The contract accepts prices only from OutLayer for a pinned version; it does not verify the TDX quote itself.
            </li>
            <li>
              <b>Sources can be wrong together.</b> The median of several public APIs, needing at least three, not proof that a price is true.
            </li>
            <li>
              <b>Quotes, not payments.</b> Cross-chain pay-in is a quote on testnet; chain signatures move test ETH only.
            </li>
            <li>
              <b>No upgrades.</b> A keyless contract cannot be fixed; a broken source means markets settle at the midpoint.
            </li>
          </ul>
        </section>
      </div>

      <section className="final">
        <h2>Lock a price, not a provider.</h2>
        <p>Open the app with a guest account, or replay a recorded market step by step.</p>
        <div className="hero-cta">
          <a className="btn" href="#/app">
            Open the app →
          </a>
          <a className="btn btn-ghost" href="?demo=scripted#/app">
            Watch the scripted demo
          </a>
        </div>
      </section>
      <footer className="lp-foot">
        <div className="lp-foot-in">
          <span>Flatrate · built at the NEAR Hackathon · NEAR testnet</span>
          <span>
            <a href={REPO} target="_blank" rel="noreferrer">
              GitHub
            </a>{" "}
            ·{" "}
            <a href={`${REPO}/blob/main/README.md`} target="_blank" rel="noreferrer">
              README
            </a>{" "}
            ·{" "}
            <a href="https://github.com/Stella112/facedown" target="_blank" rel="noreferrer">
              Also by the builder: Facedown
            </a>
          </span>
        </div>
      </footer>
    </div>
  );
}
