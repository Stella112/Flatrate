// Vercel function (project Root Directory: web).
// GET /api/tx?hash=<tx hash>&signer=<account id>
//   -> the NEAR RPC node's own EXPERIMENTAL_tx_status response for that transaction, unchanged.
// A finished transaction never changes, so a successful answer is cached at the CDN. The Proof page
// reads every observation transaction; without this, each visitor's browser hits the public archival
// node's rate limit and the last rows take minutes. No keys, read-only, one RPC method.
const NODES = ["https://archival-rpc.testnet.fastnear.com", "https://rpc.testnet.fastnear.com"];
const HASH = /^[1-9A-HJ-NP-Za-km-z]{43,44}$/;
const ACCOUNT = /^[a-z0-9._-]{2,64}$/;

export default async function handler(req, res) {
  res.setHeader("Cache-Control", "no-store");
  if (req.method !== "GET") return res.status(405).json({ error: "GET" });
  const hash = String(req.query.hash || "");
  const signer = String(req.query.signer || "");
  if (!HASH.test(hash) || !ACCOUNT.test(signer)) return res.status(400).json({ error: "hash and signer required" });
  let last = "no answer";
  for (const [i, url] of [NODES[0], NODES[0], NODES[1]].entries()) {
    if (i) await new Promise((r) => setTimeout(r, 1200));
    try {
      const r = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: "flatrate", method: "EXPERIMENTAL_tx_status", params: { tx_hash: hash, sender_account_id: signer, wait_until: "FINAL" } }),
        signal: AbortSignal.timeout(8_000),
      });
      const body = await r.json();
      if (body.result && body.result.status && (body.result.final_execution_status ?? "FINAL") === "FINAL") {
        res.setHeader("Cache-Control", "public, max-age=300, s-maxage=31536000");
        return res.status(200).json({ result: body.result, node: url });
      }
      last = body.error ? JSON.stringify(body.error).slice(0, 200) : `status ${r.status}`;
    } catch (e) {
      last = String(e && e.message ? e.message : e).slice(0, 200);
    }
  }
  return res.status(502).json({ error: last });
}
