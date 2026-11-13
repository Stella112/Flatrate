// Vercel function (project Root Directory: web).
// GET  /api/quote -> USDC tokens on EVM chains (from 1Click /v0/tokens) and which providers are configured
// POST /api/quote {provider: "1click" | "aurora", originAsset, amount}
//   -> a dry-run quote (dry: true: nothing is sent, no deposit address) for USDC on an EVM chain
//      to USDC on NEAR, from NEAR Intents 1Click or from the Aurora Intents API.
// Neither has a testnet; quotes are mainnet prices. Keys stay server-side:
//   ONECLICK_JWT (optional; avoids 1Click's 0.1% fee on live swaps) and
//   AURORA_INTENTS_API_KEY (required for the Aurora quote; from portal.intents.aurora.dev).
const ONECLICK = "https://1click.chaindefuser.com";
const AURORA = "https://intents-api.aurora.dev";
const DEST_USDC_NEAR = "nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1";
// The example EVM address from 1Click's OpenAPI spec (FACTS.md section 6); valid for EVM origins only.
const EXAMPLE_REFUND_EVM = "0x2527D02599Ba641c19FEa793cD0F167589a0f10D";
const EVM_CHAINS = new Set(["eth", "arb", "base", "gnosis", "pol", "avax", "bsc"]);

async function originTokens() {
  const r = await fetch(`${ONECLICK}/v0/tokens`, { signal: AbortSignal.timeout(15_000) });
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
    if (req.method === "GET") {
      return res.status(200).json({ tokens, destination: DEST_USDC_NEAR, providers: { "1click": true, aurora: !!process.env.AURORA_INTENTS_API_KEY } });
    }
    if (req.method !== "POST") return res.status(405).json({ error: "GET or POST" });

    const body = typeof req.body === "string" ? JSON.parse(req.body || "{}") : req.body || {};
    const provider = body.provider === "aurora" ? "aurora" : "1click";
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
    let url;
    if (provider === "aurora") {
      const key = process.env.AURORA_INTENTS_API_KEY;
      if (!key) return res.status(503).json({ provider, error: "AURORA_INTENTS_API_KEY is not set on this deployment." });
      // POST /api/quote/{apiKey} (docs.intents.aurora.dev, Swap API reference "Request a quote")
      url = `${AURORA}/api/quote/${encodeURIComponent(key)}`;
    } else {
      if (process.env.ONECLICK_JWT) headers.Authorization = `Bearer ${process.env.ONECLICK_JWT}`;
      url = `${ONECLICK}/v0/quote`;
    }
    // Both answer an error after their own internal timeout when no quote comes back; stop at 28 s.
    const r = await fetch(url, { method: "POST", headers, body: JSON.stringify(quoteRequest), signal: AbortSignal.timeout(28_000) });
    const out = await r.json().catch(() => ({}));
    return res.status(r.ok ? 200 : 502).json({ provider, status: r.status, token, quote: out.quote || null, error: r.ok ? undefined : out });
  } catch (e) {
    return res.status(502).json({ error: e instanceof Error ? e.message : String(e) });
  }
}
