// GET  /api/quote          -> USDC tokens on EVM chains from 1Click (/v0/tokens)
// POST /api/quote {originAsset, amount} -> 1Click /v0/quote with dry: true (quote only, nothing is sent)
// 1Click has no testnet; quotes are mainnet prices. If ONECLICK_JWT is set in the hosting
// provider it is sent server-side only (it avoids 1Click's 0.1% fee on live swaps).
const BASE = "https://1click.chaindefuser.com";
const DEST_USDC_NEAR = "nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
// The example EVM address from 1Click's OpenAPI spec (FACTS.md section 6); valid for EVM origins only.
const EXAMPLE_REFUND_EVM = "0x2527D02599Ba641c19FEa793cD0F167589a0f10D";
const EVM_CHAINS = new Set(["eth", "arb", "base", "gnosis", "pol", "avax", "bsc"]);

async function originTokens() {
  const r = await fetch(`${BASE}/v0/tokens`);
  if (!r.ok) throw new Error(`1Click tokens ${r.status}`);
  const all = await r.json();
  return all
    .filter((t) => t.symbol === "USDC" && EVM_CHAINS.has(t.blockchain))
    .map((t) => ({ assetId: t.assetId, blockchain: t.blockchain, symbol: t.symbol, decimals: t.decimals, price: t.price }));
}

export default async function handler(req, res) {
  res.setHeader("Cache-Control", "no-store");
  try {
    const tokens = await originTokens();
    if (req.method === "GET") return res.status(200).json({ tokens, destination: DEST_USDC_NEAR });
    if (req.method !== "POST") return res.status(405).json({ error: "GET or POST" });

    const body = typeof req.body === "string" ? JSON.parse(req.body || "{}") : req.body || {};
    const token = tokens.find((t) => t.assetId === body.originAsset);
    if (!token) return res.status(400).json({ error: "originAsset must be one of the listed USDC tokens" });
    const amount = String(body.amount || "");
    if (!/^\d+$/.test(amount) || BigInt(amount) <= 0n || BigInt(amount) > 10n ** BigInt(token.decimals) * 100_000n)
      return res.status(400).json({ error: "amount must be a positive integer in the token's smallest unit, up to 100,000 USDC" });

    const quoteRequest = {
      dry: true,
      swapType: "EXACT_INPUT",
      slippageTolerance: 100,
      originAsset: token.assetId,
      depositType: "ORIGIN_CHAIN",
      destinationAsset: DEST_USDC_NEAR,
      amount,
      refundTo: EXAMPLE_REFUND_EVM,
      refundType: "ORIGIN_CHAIN",
      recipient: "example.near",
      recipientType: "DESTINATION_CHAIN",
      deadline: new Date(Date.now() + 10 * 60_000).toISOString(),
    };
    const headers = { "Content-Type": "application/json" };
    if (process.env.ONECLICK_JWT) headers.Authorization = `Bearer ${process.env.ONECLICK_JWT}`;
    // 1Click answers 500 after its own 25 s internal timeout when no quote comes back; stop at 28 s.
    const r = await fetch(`${BASE}/v0/quote`, { method: "POST", headers, body: JSON.stringify(quoteRequest), signal: AbortSignal.timeout(28_000) });
    const out = await r.json().catch(() => ({}));
    return res.status(r.ok ? 200 : 502).json({ status: r.status, token, quote: out.quote || null, error: r.ok ? undefined : out });
  } catch (e) {
    return res.status(502).json({ error: e instanceof Error ? e.message : String(e) });
  }
}
