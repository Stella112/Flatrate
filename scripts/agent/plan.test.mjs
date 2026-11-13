// Run: node --test scripts/agent/
import { test } from "node:test";
import assert from "node:assert/strict";
import { planBid, fairLongCents, referencePrice } from "./plan.mjs";

const market = { id: "20", floor_micros: "2400000", cap_micros: "5600000" }; // band $3.20
const forecast = { gpu_hours: 40, hedge_ratio: 0.5, max_premium_cents: 3, max_spend_fusd: 50 };

test("fair Long price is the reference's position in the band, clamped to 1..99", () => {
  assert.equal(fairLongCents(3_980_000n, 2_400_000n, 5_600_000n), 49); // 1.58 / 3.20 = 49.4%
  assert.equal(fairLongCents(1_000_000n, 2_400_000n, 5_600_000n), 1);
  assert.equal(fairLongCents(9_000_000n, 2_400_000n, 5_600_000n), 99);
  assert.throws(() => fairLongCents(1n, 5n, 5n));
});

test("qty hedges hedge_ratio of the GPU-hours across the band; limit is fair + premium", () => {
  const p = planBid(market, 3_980_000n, forecast, 100_000_000n);
  assert.equal(p.qty, 64n); // 40 h x $3.20 x 0.5
  assert.equal(p.limitCents, 52);
  assert.equal(p.costMicros, 64n * 52n * 10_000n); // 33.28 fUSD
  assert.equal(p.escrowMicros, 40_000_000n); // rounded up to 40 fUSD, within 50
});

test("escrow always covers the bid and never exceeds the budget or the balance", () => {
  for (let i = 0; i < 2000; i++) {
    const f = { gpu_hours: 1 + Math.random() * 500, hedge_ratio: Math.random() || 0.1, max_premium_cents: Math.floor(Math.random() * 10), max_spend_fusd: 1 + Math.random() * 200 };
    const bal = BigInt(Math.floor(Math.random() * 300_000_000));
    const ref = BigInt(Math.floor(Math.random() * 10_000_000));
    const p = planBid(market, ref, f, bal);
    if (p.skip) continue;
    assert.ok(p.qty > 0n);
    assert.ok(p.limitCents >= 1 && p.limitCents <= 99);
    assert.ok(p.escrowMicros >= p.qty * BigInt(p.limitCents) * 10_000n, "escrow covers qty x limit (contract rule)");
    assert.ok(p.escrowMicros <= bal, "escrow <= balance");
    assert.ok(p.escrowMicros <= BigInt(Math.floor(f.max_spend_fusd * 1e6)), "escrow <= max_spend_fusd");
  }
});

test("budget caps qty; an empty budget skips", () => {
  const p = planBid(market, 3_980_000n, { ...forecast, max_spend_fusd: 10 }, 100_000_000n);
  assert.equal(p.qty, 19n); // floor(10 / 0.52)
  assert.ok(planBid(market, 3_980_000n, forecast, 0n).skip);
  assert.ok(planBid(market, 3_980_000n, { ...forecast, hedge_ratio: 0 }, 100_000_000n).skip);
});

test("reference price prefers the previous settled average, then its latest price, then the forecast", () => {
  assert.equal(referencePrice(market, { id: "11", settled_average_micros: "3980000" }, forecast).priceMicros, 3_980_000n);
  const prev = { id: "12", observations: [{ price_micros: "4000000", at: "1" }, { price_micros: "4100000", at: "2" }] };
  assert.equal(referencePrice(market, prev, forecast).priceMicros, 4_100_000n);
  assert.equal(referencePrice(market, null, { expected_price_micros: 3_500_000 }).priceMicros, 3_500_000n);
  assert.equal(referencePrice(market, null, {}).priceMicros, 4_000_000n);
});
