// SCRIPTED DEMO MODE (/?demo=scripted). Replays testnet market 2 from fixtures/demo/market-2.json,
// which scripts/record-demo-fixture.mjs recorded from public RPC. No keys, no transactions.
// The market state at each step is derived from the recorded final state and the recorded
// events of the steps so far; every step links to its real testnet transaction.
import fixture from "../../fixtures/demo/market-2.json";
import quoteFixture from "../../fixtures/1click-quote.json";
import type { Bid, Config, Market, OracleOutput } from "./types";

export type DemoStep = {
  label: string;
  hash: string;
  signer: string;
  ok: boolean;
  events: { standard: string; event: string; data: Record<string, unknown> }[];
  oracle: OracleOutput | null;
  outlayer_charged_yocto: string | null;
};

const final = fixture.market as unknown as Market;
const finalBids = fixture.bids as unknown as Bid[];
export const demoSteps = fixture.steps as unknown as DemoStep[];
export const demoConfig = fixture.config as unknown as Config;
export const demoRecordedAt = fixture.recorded_at;
export const demoQuote = quoteFixture as unknown as Record<string, any>;

const has = (upTo: number, event: string) =>
  demoSteps.slice(0, upTo + 1).filter((s) => s.events.some((e) => e.event === event)).length;

/** Market as it was after step `k` (0-based). */
export function demoMarket(k: number): Market {
  const commits = has(k, "bid_committed");
  const cleared = has(k, "auction_cleared") > 0;
  const obs = has(k, "observation_recorded");
  const settled = has(k, "market_settled") > 0;
  const redeemed = has(k, "redeem") > 0;
  return {
    ...final,
    status: settled ? final.status : cleared ? "Open" : "Auction",
    bid_count: commits,
    observations: final.observations.slice(0, obs),
    opening_price_cents: cleared ? final.opening_price_cents : null,
    pairs: cleared ? final.pairs : "0",
    collateral: redeemed ? "0" : cleared ? final.pairs : "0",
  };
}

export function demoBids(k: number): Bid[] {
  const commits = has(k, "bid_committed");
  const reveals = has(k, "bid_revealed");
  const cleared = has(k, "auction_cleared") > 0;
  return finalBids.slice(0, commits).map((b, i) => ({
    ...b,
    revealed: i < reveals ? b.revealed : null,
    filled_qty: cleared ? b.filled_qty : "0",
    cost: cleared ? b.cost : "0",
  }));
}

/** Oracle outputs recorded for the observations visible at step k, by pending id. */
export function demoOracle(k: number): Map<string, { hash: string; oracle: OracleOutput; charged: string | null }> {
  const m = new Map<string, { hash: string; oracle: OracleOutput; charged: string | null }>();
  for (const s of demoSteps.slice(0, k + 1)) {
    const rec = s.events.find((e) => e.event === "observation_recorded");
    if (rec && s.oracle) m.set(String(rec.data.pending_id), { hash: s.hash, oracle: s.oracle, charged: s.outlayer_charged_yocto });
  }
  return m;
}
