// Testnet only. Account names are the deployed ones (FACTS.md section 9).
export const NETWORK = "testnet";
export const RPC_URL = "https://rpc.testnet.fastnear.com";
// Transactions older than a few epochs are only on archival nodes.
export const ARCHIVAL_RPC_URL = "https://archival-rpc.testnet.fastnear.com";
// Public NearBlocks API v3 (no key): used only to list the contract's transactions.
export const NEARBLOCKS_API = "https://api-testnet.nearblocks.io/v3";
export const EXPLORER_TX = "https://testnet.nearblocks.io/txns/";
export const EXPLORER_ACCOUNT = "https://testnet.nearblocks.io/address/";

/** One Flatrate contract per index (extra 2 deployed the same contract for LLM token prices). */
export type IndexInfo = {
  key: "h100" | "tokens" | "sealed";
  contractId: string;
  name: string; // nav label
  unit: string; // what one price is
  sources: string; // where prices come from
  /** Bids are sealed to an OutLayer enclave key and cleared inside the enclave (no reveal step). */
  privateAuction?: boolean;
};
export const INDEXES: IndexInfo[] = [
  {
    key: "h100",
    contractId: "flatrate.flatrate-maris.testnet",
    name: "H100 GPU-hour",
    unit: "USD per H100 GPU-hour",
    sources: "five public GPU price APIs (Akash, Vast.ai, RunPod, Shadeform, Verda)",
  },
  {
    key: "tokens",
    contractId: "tokens.flatrate-maris.testnet",
    name: "LLM tokens",
    unit: "USD per 1M output tokens of Llama 3.3 70B",
    sources: "OpenRouter's public price list for Llama 3.3 70B, one price per provider (seven providers)",
  },
  {
    key: "sealed",
    contractId: "sealed.flatrate-maris.testnet",
    name: "H100 private auction",
    unit: "USD per H100 GPU-hour",
    sources: "five public GPU price APIs (Akash, Vast.ai, RunPod, Shadeform, Verda)",
    privateAuction: true,
  },
];
/** `?index=tokens` picks the token-price contract; scripted demo mode always replays the H100 market. */
export const INDEX: IndexInfo = (() => {
  const q = new URLSearchParams(window.location.search);
  const k = q.get("demo") === "scripted" ? "h100" : q.get("index");
  return INDEXES.find((i) => i.key === k) ?? INDEXES[0];
})();
export const FLATRATE_ID = INDEX.contractId;
export const FUSD_ID = "fusd.flatrate-maris.testnet";
export const OUTLAYER_ID = "outlayer.testnet";

export const ONE = 1_000_000n; // 1 fUSD = 1 claim = 1_000_000 units
export const TGAS = 1_000_000_000_000n;

export const isScriptedDemo = (): boolean =>
  new URLSearchParams(window.location.search).get("demo") === "scripted";
