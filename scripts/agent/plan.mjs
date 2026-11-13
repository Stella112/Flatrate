// Pure bid planning for the hedging agent. No network, no keys: tested by plan.test.mjs.
//
// The agent buys GPU time, so a higher price costs it more. It hedges by buying Long claims:
// one Long pays $1 x (avg - floor) / (cap - floor), so to offset `h` GPU-hours of price risk
// across the band it needs h x (cap - floor) / $1 claims. It hedges `hedge_ratio` of that.

const MICRO = 1_000_000n;

/**
 * Reference price for a market that is still in its auction: the previous market of the same
 * template (settled average, else its latest recorded slot price), else the forecast's
 * `expected_price_micros`, else the middle of the band.
 * @returns {{ priceMicros: bigint, source: string }}
 */
export function referencePrice(market, previous, forecast) {
  if (previous?.settled_average_micros) {
    return { priceMicros: BigInt(previous.settled_average_micros), source: `market ${previous.id} settled average` };
  }
  const obs = previous?.observations?.filter((o) => o && o.price_micros) ?? [];
  if (obs.length) {
    const last = obs.reduce((a, b) => (BigInt(b.at) > BigInt(a.at) ? b : a));
    return { priceMicros: BigInt(last.price_micros), source: `market ${previous.id} latest price` };
  }
  if (forecast.expected_price_micros) {
    return { priceMicros: BigInt(forecast.expected_price_micros), source: "forecast expected_price_micros" };
  }
  return { priceMicros: (BigInt(market.floor_micros) + BigInt(market.cap_micros)) / 2n, source: "middle of the band" };
}

/** Fair Long price in whole cents (1..=99) for a reference price inside or outside the band. */
export function fairLongCents(priceMicros, floorMicros, capMicros) {
  const f = BigInt(floorMicros), c = BigInt(capMicros), p = BigInt(priceMicros);
  if (c <= f) throw new Error("cap must be above floor");
  let cents = ((p - f) * 100n + (c - f) / 2n) / (c - f); // rounded
  if (cents < 1n) cents = 1n;
  if (cents > 99n) cents = 99n;
  return Number(cents);
}

/**
 * Plans one sealed Long bid.
 * forecast: { gpu_hours, hedge_ratio, max_premium_cents, max_spend_fusd, expected_price_micros? }
 * balanceMicros: the agent's fUSD balance.
 * @returns {{ side: "Long", qty: bigint, limitCents: number, costMicros: bigint, escrowMicros: bigint,
 *             fairCents: number } | { skip: string }}
 */
export function planBid(market, refPriceMicros, forecast, balanceMicros) {
  const floor = BigInt(market.floor_micros), cap = BigInt(market.cap_micros);
  const hours = Number(forecast.gpu_hours), ratio = Number(forecast.hedge_ratio);
  if (!(hours > 0) || !(ratio > 0 && ratio <= 1)) return { skip: "forecast needs gpu_hours > 0 and 0 < hedge_ratio <= 1" };

  const fairCents = fairLongCents(refPriceMicros, floor, cap);
  const limitCents = Math.min(99, fairCents + Math.max(0, Math.floor(Number(forecast.max_premium_cents ?? 0))));

  // Claims needed: hours x band width in dollars x ratio, rounded down (micro-precision maths).
  const ratioMicros = BigInt(Math.round(ratio * 1e6));
  const hoursMicros = BigInt(Math.round(hours * 1e6));
  let qty = (hoursMicros * (cap - floor) * ratioMicros) / (MICRO * MICRO * MICRO);

  const perClaim = BigInt(limitCents) * 10_000n; // micro-fUSD escrow per claim at the limit
  const maxSpend = BigInt(Math.floor(Number(forecast.max_spend_fusd) * 1e6));
  const spendable = maxSpend < BigInt(balanceMicros) ? maxSpend : BigInt(balanceMicros);
  if (qty * perClaim > spendable) qty = spendable / perClaim;
  if (qty <= 0n) return { skip: `no budget: spendable ${spendable} micro-fUSD < ${perClaim} per claim` };

  const costMicros = qty * perClaim;
  // Round the deposit up to the next 10 fUSD (within the spendable budget) so it does not reveal qty x limit.
  const ten = 10n * MICRO;
  let escrowMicros = ((costMicros + ten - 1n) / ten) * ten;
  if (escrowMicros > spendable) escrowMicros = spendable;
  if (escrowMicros < costMicros) escrowMicros = costMicros;

  return { side: "Long", qty, limitCents, costMicros, escrowMicros, fairCents };
}
