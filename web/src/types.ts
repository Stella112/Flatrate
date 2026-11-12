// JSON shapes returned by the Flatrate contract views (contracts/flatrate/src/lib.rs).
export type Side = "Long" | "Short";

export type MarketStatus = "Auction" | "Open" | "Void" | { Settled: { long_payout_micros: number } };

export type Observation = {
  price_micros: string;
  sources_ok: number;
  at: string;
  outlayer_request: string;
  pending_id: string;
};

export type Market = {
  id: string;
  index: string;
  floor_micros: string;
  cap_micros: string;
  auction_commit_end: string;
  auction_reveal_end: string;
  period_start: string;
  period_end: string;
  min_observations: number;
  void_after: string;
  status: MarketStatus;
  observations: Observation[];
  oracle_version_key: string;
  oracle_input: string;
  opening_price_cents: number | null;
  pairs: string;
  collateral: string;
  bid_count: number;
};

export type Bid = {
  index: number;
  bidder: string;
  hash: string;
  escrow: string;
  committed_at: string;
  revealed: { side: Side; qty: string; limit_cents: number } | null;
  filled_qty: string;
  cost: string;
};

export type Config = {
  owner_id: string;
  fusd_id: string;
  outlayer_id: string;
  oracle_project_id: string;
  oracle_version_key: string;
  oracle_sources: string[];
  resource_limits: { max_instructions: string; max_memory_mb: number; max_execution_seconds: string };
  next_pending_id: string;
  market_count: string;
};

export type NftToken = {
  token_id: string;
  owner_id: string;
  metadata: { title: string | null; description: string | null; extra: string | null } | null;
};

export type Note = { market_id: string; side: Side; amount: string };

export type OracleOutput = {
  index: string;
  price_micros: number | null;
  sources: { name: string; price_micros: number | null; error: string | null }[];
  fetched_at: number;
  request_id?: string | null;
};
