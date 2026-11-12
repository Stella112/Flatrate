// Testnet only. Account names are the deployed ones (FACTS.md section 9).
export const NETWORK = "testnet";
export const RPC_URL = "https://rpc.testnet.fastnear.com";
// Transactions older than a few epochs are only on archival nodes.
export const ARCHIVAL_RPC_URL = "https://archival-rpc.testnet.fastnear.com";
// Public NearBlocks API v3 (no key): used only to list the contract's transactions.
export const NEARBLOCKS_API = "https://api-testnet.nearblocks.io/v3";
export const EXPLORER_TX = "https://testnet.nearblocks.io/txns/";
export const EXPLORER_ACCOUNT = "https://testnet.nearblocks.io/address/";

export const FLATRATE_ID = "flatrate.flatrate-maris.testnet";
export const FUSD_ID = "fusd.flatrate-maris.testnet";
export const OUTLAYER_ID = "outlayer.testnet";

export const ONE = 1_000_000n; // 1 fUSD = 1 claim = 1_000_000 units
export const TGAS = 1_000_000_000_000n;

export const isScriptedDemo = (): boolean =>
  new URLSearchParams(window.location.search).get("demo") === "scripted";
