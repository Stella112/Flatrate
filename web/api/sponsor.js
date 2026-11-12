// Vercel function (project Root Directory: web). POST /api/sponsor {"account_id": "<64 hex>"}
// Creates the guest's implicit account by sending it 0.5 testnet NEAR, registers it on fUSD and
// gives it 100 fUSD (the sponsor mints them from the public faucet and passes them on).
// Env (set in the hosting provider, never in the repo): SPONSOR_ACCOUNT_ID, SPONSOR_PRIVATE_KEY.
// Rate limit: 3 accounts per IP per hour, kept in this function instance's memory only, so it is
// best-effort (a cold start or another instance starts empty). Testnet only.
import { Account, JsonRpcProvider, actions } from "near-api-js";

const RPC_URL = process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com";
const FUSD_ID = process.env.FUSD_ID || "fusd.flatrate-maris.testnet";
const GUEST_NEAR = 500_000_000_000_000_000_000_000n; // 0.5 NEAR
const FUSD_REGISTRATION = 1_250_000_000_000_000_000_000n; // storage_balance_bounds on fUSD
const GUEST_FUSD = "100000000"; // 100 fUSD
const MIN_SPONSOR_BALANCE = 1_000_000_000_000_000_000_000_000n; // keep 1 NEAR
const TGAS = 1_000_000_000_000n;
const PER_IP_PER_HOUR = 3;

const hits = new Map(); // ip -> [timestamps]

function rateLimited(ip) {
  const now = Date.now();
  const recent = (hits.get(ip) || []).filter((t) => now - t < 3_600_000);
  if (recent.length >= PER_IP_PER_HOUR) return true;
  recent.push(now);
  hits.set(ip, recent);
  return false;
}

async function viewAccount(provider, accountId) {
  try {
    return await provider.viewAccount({ accountId });
  } catch {
    return null;
  }
}

const txHash = (r) => r?.transaction?.hash || r?.transaction_outcome?.id;
const failed = (r) => r?.status && typeof r.status === "object" && "Failure" in r.status;

export default async function handler(req, res) {
  res.setHeader("Cache-Control", "no-store");
  if (req.method !== "POST") return res.status(405).json({ error: "POST only" });
  const { SPONSOR_ACCOUNT_ID, SPONSOR_PRIVATE_KEY } = process.env;
  if (!SPONSOR_ACCOUNT_ID || !SPONSOR_PRIVATE_KEY) return res.status(503).json({ error: "Sponsor is not configured on this deployment." });

  const body = typeof req.body === "string" ? JSON.parse(req.body || "{}") : req.body || {};
  const accountId = String(body.account_id || "").toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(accountId)) return res.status(400).json({ error: "account_id must be a 64-character hex implicit account id" });

  const ip = String(req.headers["x-forwarded-for"] || req.socket?.remoteAddress || "unknown").split(",")[0].trim();
  if (rateLimited(ip)) return res.status(429).json({ error: `Limit of ${PER_IP_PER_HOUR} guest accounts per hour from one address.` });

  const provider = new JsonRpcProvider({ url: RPC_URL });
  if (await viewAccount(provider, accountId)) return res.status(409).json({ error: "This guest account already exists." });

  const sponsorAcc = await viewAccount(provider, SPONSOR_ACCOUNT_ID);
  if (!sponsorAcc || BigInt(sponsorAcc.amount) < MIN_SPONSOR_BALANCE + GUEST_NEAR)
    return res.status(503).json({ error: "The sponsor is out of testnet NEAR. Try the scripted demo (?demo=scripted)." });

  const sponsor = new Account(SPONSOR_ACCOUNT_ID, provider, SPONSOR_PRIVATE_KEY);
  try {
    // 1. Sending NEAR to an implicit account id creates the account.
    const t1 = await sponsor.signAndSendTransaction({
      receiverId: accountId,
      actions: [actions.transfer(GUEST_NEAR)],
      throwOnFailure: false,
      waitUntil: "FINAL",
    });
    if (failed(t1)) return res.status(502).json({ error: "Funding transfer failed", txs: [txHash(t1)] });

    // 2. Mint 100 fUSD to the sponsor from the faucet, register the guest, pass the fUSD on.
    const t2 = await sponsor.signAndSendTransaction({
      receiverId: FUSD_ID,
      actions: [
        actions.functionCall("faucet", { amount: GUEST_FUSD }, 30n * TGAS, 0n),
        actions.functionCall("storage_deposit", { account_id: accountId, registration_only: true }, 30n * TGAS, FUSD_REGISTRATION),
        actions.functionCall("ft_transfer", { receiver_id: accountId, amount: GUEST_FUSD, memo: "flatrate guest" }, 30n * TGAS, 1n),
      ],
      throwOnFailure: false,
      waitUntil: "FINAL",
    });
    const txs = [txHash(t1), txHash(t2)];
    if (failed(t2)) return res.status(502).json({ error: "Account created, but the fUSD gift failed. Use the faucet button.", txs });
    return res.status(200).json({ message: `Guest account funded: 0.5 NEAR and 100 fUSD (tx ${txs.map((t) => t?.slice(0, 8)).join(", ")}).`, txs });
  } catch (e) {
    return res.status(502).json({ error: `Sponsor error: ${e instanceof Error ? e.message : String(e)}` });
  }
}
