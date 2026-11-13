// SCRIPTED DEMO MODE (/?demo=scripted). Replays a finished testnet market from
// fixtures/demo/market-2.json, which scripts/record-demo-fixture.mjs recorded from public RPC.
// No keys, no transactions. The market at each step is derived from the recorded final state
// and the recorded events of the steps so far; every step links to its real testnet transaction.
// The recording comes from the first contract version (observations without slots); it is shown
// here in the current shape with four 4-minute slots, which is where its three prices fell
// (19:32, 19:35 and 19:39 UTC in a 19:31-19:47 period).
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

// deno-lint-ignore no-explicit-any
const raw = fixture.market as any;
const finalBids = fixture.bids as unknown as Bid[];
export const demoSteps = fixture.steps as unknown as DemoStep[];
export const demoRecordedAt = fixture.recorded_at;
export const demoQuote = quoteFixture as unknown as Record<string, any>;
// deno-lint-ignore no-explicit-any
const rawCfg = fixture.config as any;
export const demoConfig = { ...rawCfg, next_market_id: rawCfg.market_count } as Config;

const SLOTS = 4;
const settleEvent = demoSteps.flatMap((s) => s.events).find((e) => e.event === "market_settled");
const final: Market = {
  ...raw,
  template: "DEMO",
  slot_ns: String((BigInt(raw.period_end) - BigInt(raw.period_start)) / BigInt(SLOTS)),
  num_slots: SLOTS,
  min_filled_slots: raw.min_observations,
  filled_slots: raw.observations.length,
  settled_average_micros: settleEvent ? String(settleEvent.data.avg_micros) : null,
  created_by: rawCfg.owner_id,
  // deno-lint-ignore no-explicit-any
  observations: raw.observations.map((o: any, i: number) => ({ ...o, slot: i })),
};

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
    filled_slots: obs,
    settled_average_micros: settled ? final.settled_average_micros : null,
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
