// JSON shapes returned by the Flatrate contract views (contracts/flatrate/src/lib.rs, v2).
export type Side = "Long" | "Short";
export type Template = "DEMO" | "WEEKLY";

export type MarketStatus = "Auction" | "Open" | "Void" | { Settled: { long_payout_micros: number } };

export type Observation = {
  slot: number;
  price_micros: string;
  sources_ok: number;
  at: string;
  outlayer_request: string;
  pending_id: string;
};

export type Market = {
  id: string;
  template: Template;
  index: string;
  floor_micros: string;
  cap_micros: string;
  auction_commit_end: string;
  auction_reveal_end: string;
  period_start: string;
  period_end: string;
  slot_ns: string;
  num_slots: number;
  min_filled_slots: number;
  void_after: string;
  status: MarketStatus;
  filled_slots: number;
  settled_average_micros: string | null;
  observations: Observation[];
  oracle_version_key: string;
  oracle_input: string;
  opening_price_cents: number | null;
  pairs: string;
  collateral: string;
  bid_count: number;
  created_by: string;
};

export type SlotInfo = {
  market_id: string;
  num_slots: number;
  current_slot: number | null;
  current_filled: boolean;
  current_pending: boolean;
  filled_slots: number;
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

export type TemplateConfig = {
  commit_ns: string;
  reveal_ns: string;
  period_ns: string;
  slot_ns: string;
  min_filled_slots: number;
  void_after_ns: string;
};

export type Config = {
  owner_id: string;
  fusd_id: string;
  outlayer_id: string;
  oracle_project_id: string;
  oracle_version_key: string;
  oracle_sources: string[];
  resource_limits: { max_instructions: string; max_memory_mb: number; max_execution_seconds: string };
  demo_template: TemplateConfig;
  weekly_template: TemplateConfig;
  observation_deposit: string;
  observation_budget: string;
  next_pending_id: string;
  next_market_id: string;
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

/** OutLayer's attestation record for one execution (GET /attestations/by-request/{id}). */
export type Attestation = {
  request_id: number;
  transaction_hash: string;
  wasm_hash: string;
  executed_wasm_sha256: string;
  input_hash: string;
  output_hash: string;
  worker_measurement: string;
  tdx_quote: string;
  project_id: string;
  timestamp: number;
};
