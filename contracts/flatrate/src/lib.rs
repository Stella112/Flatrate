//! Flatrate: GPU price claims settled on an OutLayer-attested median.
//!
//! Markets, the sealed commit-reveal opening auction, the claims ledger, observations through
//! the OutLayer callback, settlement, redemption and the void path. Cover Notes (Phase 4)
//! build on this file.

use flatrate_shared as math;
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::{self, json};
use near_sdk::store::{LookupMap, Vector};
use near_sdk::{
    env, near, require, AccountId, BorshStorageKey, Gas, GasWeight, NearToken, PanicOnDefault,
    Promise, PromiseError, PromiseOrValue,
};

/// Gas for an fUSD `ft_transfer`.
const GAS_FT_TRANSFER: Gas = Gas::from_tgas(10);
/// Gas for the callback after an fUSD payout.
const GAS_ON_PAYOUT: Gas = Gas::from_tgas(10);
/// Static gas for the OutLayer call. OutLayer reserves 50 TGas of what it receives for its
/// response (near-outlayer `contract/src/lib.rs:28`); unused gas is also routed to it by weight.
const GAS_OUTLAYER_STATIC: Gas = Gas::from_tgas(100);
/// Static gas for `on_observation`.
const GAS_ON_OBSERVATION: Gas = Gas::from_tgas(30);
/// Largest OutLayer result we read, in bytes.
const MAX_ORACLE_OUTPUT: usize = 16_384;
/// Minimum number of sources that must return a price.
pub const MIN_SOURCES_OK: u8 = 3;
/// Most bids one auction accepts, so `clear_auction` fits in one transaction's gas.
pub const MAX_BIDS: u32 = 200;
/// One whole claim in claim units (6 decimals, like fUSD).
const CLAIM_UNITS: u128 = math::PAIR_MICROS as u128;

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Markets,
    Claims,
    Pending,
    Bids,
    BidIndex,
    Refunds,
}

/// A sealed bid. `hash = sha256(borsh(market_id: u64, side: Side, qty: u64, limit_cents: u8,
/// salt: String))`; `qty` is whole claims; the escrow must cover `qty x limit_cents x 10_000`.
#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct Bid {
    pub bidder: AccountId,
    pub hash: [u8; 32],
    pub escrow: u128,
    pub committed_at: u64,
    pub revealed: Option<RevealedBid>,
    /// Set by `clear_auction`.
    pub filled_qty: u64,
    pub cost: u128,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct RevealedBid {
    pub side: Side,
    pub qty: U64,
    pub limit_cents: u8,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct BidView {
    pub index: u32,
    pub bidder: AccountId,
    pub hash: String,
    pub escrow: U128,
    pub committed_at: U64,
    pub revealed: Option<RevealedBid>,
    pub filled_qty: U64,
    pub cost: U128,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

fn hex_decode_32(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let nib = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = (nib(b[2 * i])? << 4) | nib(b[2 * i + 1])?;
    }
    Some(out)
}

/// The commitment for a bid. Exposed as a view (`bid_hash`) so clients can check their hashing.
pub fn bid_commitment(market_id: u64, side: Side, qty: u64, limit_cents: u8, salt: &str) -> [u8; 32] {
    let bytes = near_sdk::borsh::to_vec(&(market_id, side, qty, limit_cents, salt.to_string()))
        .unwrap_or_else(|_| env::panic_str("ERR_BORSH"));
    env::sha256_array(&bytes)
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Long,
    Short,
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum IndexId {
    H100_HOURLY,
}

impl IndexId {
    pub fn as_str(&self) -> &'static str {
        match self {
            IndexId::H100_HOURLY => "H100_HOURLY",
        }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketStatus {
    Auction,
    Open,
    Settled { long_payout_micros: u64 },
    Void,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct Observation {
    pub price_micros: u64,
    pub sources_ok: u8,
    /// Block timestamp (ns) when the result was recorded.
    pub at: u64,
    /// OutLayer request id, as reported by the oracle component (`NEAR_REQUEST_ID`).
    pub outlayer_request: String,
    /// This contract's pending id for the request (used, so it can never be replayed).
    pub pending_id: u64,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct Market {
    pub id: u64,
    pub index: IndexId,
    pub floor_micros: u64,
    pub cap_micros: u64,
    pub auction_commit_end: u64,
    pub auction_reveal_end: u64,
    pub period_start: u64,
    pub period_end: u64,
    pub min_observations: u32,
    pub void_after: u64,
    pub status: MarketStatus,
    pub observations: Vec<Observation>,
    /// OutLayer project version pinned when the market was created. Never changes.
    pub oracle_version_key: String,
    /// JSON input sent to the oracle for this market. Never changes.
    pub oracle_input: String,
    /// Opening price of one Long claim in cents, set by the auction.
    pub opening_price_cents: Option<u8>,
    /// Pairs outstanding (= Long supply = Short supply) before settlement, in claim units.
    pub pairs: u128,
    /// fUSD held for this market, micro-units.
    pub collateral: u128,
    /// Sealed bids committed to the opening auction.
    pub bid_count: u32,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct PendingObservation {
    pub market_id: u64,
    pub caller: AccountId,
    pub deposit: NearToken,
    pub requested_at: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct ObservationView {
    pub price_micros: U64,
    pub sources_ok: u8,
    pub at: U64,
    pub outlayer_request: String,
    pub pending_id: U64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct MarketView {
    pub id: U64,
    pub index: IndexId,
    pub floor_micros: U64,
    pub cap_micros: U64,
    pub auction_commit_end: U64,
    pub auction_reveal_end: U64,
    pub period_start: U64,
    pub period_end: U64,
    pub min_observations: u32,
    pub void_after: U64,
    pub status: MarketStatus,
    pub observations: Vec<ObservationView>,
    pub oracle_version_key: String,
    pub oracle_input: String,
    pub opening_price_cents: Option<u8>,
    pub pairs: U128,
    pub collateral: U128,
    pub bid_count: u32,
}

impl From<&Market> for MarketView {
    fn from(m: &Market) -> Self {
        Self {
            id: U64(m.id),
            index: m.index,
            floor_micros: U64(m.floor_micros),
            cap_micros: U64(m.cap_micros),
            auction_commit_end: U64(m.auction_commit_end),
            auction_reveal_end: U64(m.auction_reveal_end),
            period_start: U64(m.period_start),
            period_end: U64(m.period_end),
            min_observations: m.min_observations,
            void_after: U64(m.void_after),
            status: m.status.clone(),
            observations: m
                .observations
                .iter()
                .map(|o| ObservationView {
                    price_micros: U64(o.price_micros),
                    sources_ok: o.sources_ok,
                    at: U64(o.at),
                    outlayer_request: o.outlayer_request.clone(),
                    pending_id: U64(o.pending_id),
                })
                .collect(),
            oracle_version_key: m.oracle_version_key.clone(),
            oracle_input: m.oracle_input.clone(),
            opening_price_cents: m.opening_price_cents,
            pairs: U128(m.pairs),
            collateral: U128(m.collateral),
            bid_count: m.bid_count,
        }
    }
}

/// `max_instructions`, `max_memory_mb`, `max_execution_seconds` sent to OutLayer
/// (near-outlayer `contract/src/lib.rs:259-273`).
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct ResourceLimits {
    pub max_instructions: U64,
    pub max_memory_mb: u32,
    pub max_execution_seconds: U64,
}

#[near(serializers = [json])]
pub struct ConfigView {
    pub owner_id: AccountId,
    pub fusd_id: AccountId,
    pub outlayer_id: AccountId,
    pub oracle_project_id: String,
    pub oracle_version_key: String,
    pub oracle_sources: Vec<String>,
    pub resource_limits: ResourceLimits,
    pub next_pending_id: U64,
    pub market_count: U64,
}

/// One source's result in the oracle output.
#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct OracleSource {
    pub name: String,
    #[serde(default)]
    pub price_micros: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

/// JSON printed by the oracle component (see CLAUDE.md "Oracle component").
#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct OracleOutput {
    pub index: String,
    #[serde(default)]
    pub price_micros: Option<u64>,
    pub sources: Vec<OracleSource>,
    pub fetched_at: u64,
    #[serde(default)]
    pub request_id: Option<String>,
}

/// Messages accepted in `ft_transfer_call` from fUSD.
#[near(serializers = [json])]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum TransferMsg {
    MintPair { market_id: U64 },
    /// `hash`: 64 hex characters, see `Bid`.
    CommitBid { market_id: U64, hash: String },
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimKey {
    pub market_id: u64,
    pub side: Side,
    pub account_id: AccountId,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    fusd_id: AccountId,
    outlayer_id: AccountId,
    oracle_project_id: String,
    /// Version pinned into markets created from now on.
    oracle_version_key: String,
    oracle_sources: Vec<String>,
    resource_limits: ResourceLimits,
    markets: Vector<Market>,
    claims: LookupMap<ClaimKey, u128>,
    pending: LookupMap<u64, PendingObservation>,
    next_pending_id: u64,
    /// (market_id, bid index) -> bid. Index order is commit order.
    bids: LookupMap<(u64, u32), Bid>,
    /// (market_id, bidder, hash) -> bid index, for `reveal_bid`.
    bid_index: LookupMap<(u64, AccountId, [u8; 32]), u32>,
    /// fUSD owed back to an account (unused auction escrow), withdrawn with `withdraw_refund`.
    refunds: LookupMap<AccountId, u128>,
}

fn emit(event: &str, data: serde_json::Value) {
    env::log_str(&format!(
        "EVENT_JSON:{}",
        json!({"standard": "flatrate", "version": "1.0.0", "event": event, "data": [data]})
    ));
}

#[near]
impl Contract {
    #[init]
    pub fn new(
        owner_id: AccountId,
        fusd_id: AccountId,
        outlayer_id: AccountId,
        oracle_project_id: String,
        oracle_version_key: String,
        oracle_sources: Vec<String>,
        resource_limits: ResourceLimits,
    ) -> Self {
        require!(!oracle_version_key.is_empty(), "ERR_EMPTY_VERSION");
        require!(oracle_sources.len() >= MIN_SOURCES_OK as usize, "ERR_TOO_FEW_SOURCES");
        Self {
            owner_id,
            fusd_id,
            outlayer_id,
            oracle_project_id,
            oracle_version_key,
            oracle_sources,
            resource_limits,
            markets: Vector::new(StorageKey::Markets),
            claims: LookupMap::new(StorageKey::Claims),
            pending: LookupMap::new(StorageKey::Pending),
            next_pending_id: 0,
            bids: LookupMap::new(StorageKey::Bids),
            bid_index: LookupMap::new(StorageKey::BidIndex),
            refunds: LookupMap::new(StorageKey::Refunds),
        }
    }

    // ---------- owner ----------

    fn assert_owner(&self) {
        require!(env::predecessor_account_id() == self.owner_id, "ERR_NOT_OWNER");
    }

    /// Owner-only. Creates a market in `Auction` status. All times are ns since the epoch.
    #[allow(clippy::too_many_arguments)]
    pub fn create_market(
        &mut self,
        index: IndexId,
        floor_micros: U64,
        cap_micros: U64,
        auction_commit_end: U64,
        auction_reveal_end: U64,
        period_start: U64,
        period_end: U64,
        min_observations: u32,
        void_after: U64,
    ) -> U64 {
        self.assert_owner();
        let now = env::block_timestamp();
        require!(floor_micros.0 > 0 && cap_micros.0 > floor_micros.0, "ERR_BAD_BOUNDS");
        require!(now < auction_commit_end.0, "ERR_BAD_TIMES");
        require!(auction_commit_end.0 <= auction_reveal_end.0, "ERR_BAD_TIMES");
        require!(auction_reveal_end.0 <= period_start.0, "ERR_BAD_TIMES");
        require!(period_start.0 < period_end.0, "ERR_BAD_TIMES");
        require!(period_end.0 <= void_after.0, "ERR_BAD_TIMES");
        require!(min_observations >= 1, "ERR_BAD_MIN_OBSERVATIONS");
        let id = self.markets.len() as u64;
        let oracle_input =
            json!({"index": index.as_str(), "sources": self.oracle_sources}).to_string();
        self.markets.push(Market {
            id,
            index,
            floor_micros: floor_micros.0,
            cap_micros: cap_micros.0,
            auction_commit_end: auction_commit_end.0,
            auction_reveal_end: auction_reveal_end.0,
            period_start: period_start.0,
            period_end: period_end.0,
            min_observations,
            void_after: void_after.0,
            status: MarketStatus::Auction,
            observations: vec![],
            oracle_version_key: self.oracle_version_key.clone(),
            oracle_input,
            opening_price_cents: None,
            pairs: 0,
            collateral: 0,
            bid_count: 0,
        });
        emit("market_created", json!({"market_id": U64(id)}));
        U64(id)
    }

    /// Owner-only. Sets the oracle version pinned into markets created after this call.
    /// Markets that already exist keep their pinned version.
    pub fn set_oracle_version(&mut self, version_key: String) {
        self.assert_owner();
        require!(!version_key.is_empty(), "ERR_EMPTY_VERSION");
        self.oracle_version_key = version_key;
    }

    // ---------- sealed opening auction ----------

    fn internal_commit_bid(&mut self, market_id: u64, bidder: &AccountId, escrow: u128, hash: &str) {
        let mut m = self.market(market_id);
        require!(m.status == MarketStatus::Auction, "ERR_NOT_IN_AUCTION");
        let now = env::block_timestamp();
        require!(now < m.auction_commit_end, "ERR_COMMIT_ENDED");
        require!(escrow > 0, "ERR_ZERO_AMOUNT");
        require!(m.bid_count < MAX_BIDS, "ERR_TOO_MANY_BIDS");
        let hash = hex_decode_32(hash).unwrap_or_else(|| env::panic_str("ERR_BAD_HASH"));
        let key = (market_id, bidder.clone(), hash);
        require!(!self.bid_index.contains_key(&key), "ERR_DUPLICATE_COMMIT");
        let index = m.bid_count;
        self.bids.insert(
            (market_id, index),
            Bid { bidder: bidder.clone(), hash, escrow, committed_at: now, revealed: None, filled_qty: 0, cost: 0 },
        );
        self.bid_index.insert(key, index);
        m.bid_count += 1;
        self.save(m);
        emit(
            "bid_committed",
            json!({"market_id": U64(market_id), "index": index, "bidder": bidder, "escrow": U128(escrow), "hash": hex_encode(&hash)}),
        );
    }

    /// During the reveal window. Opens the caller's commitment; valid only if the hash matches
    /// and the escrow covers `qty x limit_cents x 10_000`.
    pub fn reveal_bid(&mut self, market_id: U64, side: Side, qty: U64, limit_cents: u8, salt: String) {
        let m = self.market(market_id.0);
        require!(m.status == MarketStatus::Auction, "ERR_NOT_IN_AUCTION");
        let now = env::block_timestamp();
        require!(now >= m.auction_commit_end, "ERR_REVEAL_NOT_OPEN");
        require!(now < m.auction_reveal_end, "ERR_REVEAL_ENDED");
        require!(qty.0 > 0 && (1..=99).contains(&limit_cents), "ERR_BAD_BID");
        let bidder = env::predecessor_account_id();
        let hash = bid_commitment(market_id.0, side, qty.0, limit_cents, &salt);
        let index = self
            .bid_index
            .get(&(market_id.0, bidder.clone(), hash))
            .copied()
            .unwrap_or_else(|| env::panic_str("ERR_BAD_REVEAL"));
        let mut bid = self.bids.get(&(market_id.0, index)).cloned().unwrap();
        require!(bid.revealed.is_none(), "ERR_ALREADY_REVEALED");
        let need = qty.0 as u128 * limit_cents as u128 * math::CENT_MICROS as u128;
        require!(need <= bid.escrow, "ERR_ESCROW_TOO_LOW");
        bid.revealed = Some(RevealedBid { side, qty, limit_cents });
        self.bids.insert((market_id.0, index), bid);
        emit(
            "bid_revealed",
            json!({"market_id": market_id, "index": index, "bidder": bidder, "side": side, "qty": qty, "limit_cents": limit_cents}),
        );
    }

    /// Anyone, after the reveal window. Uniform-price clearing (`flatrate_shared::clear_auction`):
    /// matched bids get claims, every unused escrow (including unrevealed bids) is credited to
    /// the bidder's refund balance, and the market opens with the opening price recorded.
    pub fn clear_auction(&mut self, market_id: U64) {
        let mut m = self.market(market_id.0);
        require!(m.status == MarketStatus::Auction, "ERR_NOT_IN_AUCTION");
        require!(env::block_timestamp() >= m.auction_reveal_end, "ERR_REVEAL_NOT_ENDED");

        let mut bids: Vec<Bid> = (0..m.bid_count)
            .map(|i| self.bids.get(&(market_id.0, i)).cloned().unwrap())
            .collect();
        // Revealed bids go to the clearing; `id` is the commit index.
        let revealed: Vec<math::AuctionBid> = bids
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                b.revealed.as_ref().map(|r| math::AuctionBid {
                    id: i as u64,
                    side: match r.side {
                        Side::Long => math::BidSide::Long,
                        Side::Short => math::BidSide::Short,
                    },
                    qty: r.qty.0,
                    limit_cents: r.limit_cents,
                    escrow_micros: b.escrow,
                })
            })
            .collect();
        let c = math::clear_auction(&revealed);
        let mut refunded: Vec<u128> = bids.iter().map(|b| b.escrow).collect();
        for f in &c.fills {
            let i = f.id as usize;
            refunded[i] = f.refund_micros;
            if f.filled_qty > 0 {
                let side = bids[i].revealed.as_ref().unwrap().side;
                let bidder = bids[i].bidder.clone();
                self.add_claim(market_id.0, side, &bidder, f.filled_qty as u128 * CLAIM_UNITS);
                bids[i].filled_qty = f.filled_qty;
                bids[i].cost = f.cost_micros;
            }
        }
        for (i, b) in bids.into_iter().enumerate() {
            if refunded[i] > 0 {
                let cur = self.refunds.get(&b.bidder).copied().unwrap_or(0);
                self.refunds.insert(b.bidder.clone(), cur + refunded[i]);
            }
            if b.filled_qty > 0 {
                self.bids.insert((market_id.0, i as u32), b);
            }
        }
        let minted = c.matched as u128 * CLAIM_UNITS;
        m.pairs += minted;
        m.collateral += minted;
        m.opening_price_cents = c.price_cents;
        m.status = MarketStatus::Open;
        emit(
            "auction_cleared",
            json!({"market_id": market_id, "price_cents": c.price_cents, "matched": U128(minted),
                   "bids": m.bid_count, "revealed": revealed.len()}),
        );
        self.save(m);
    }

    /// Sends the caller's whole refund balance (unused auction escrow) in fUSD.
    pub fn withdraw_refund(&mut self) -> U128 {
        let account_id = env::predecessor_account_id();
        let amount = self.refunds.remove(&account_id).unwrap_or(0);
        require!(amount > 0, "ERR_NOTHING_TO_REFUND");
        emit("refund_withdrawn", json!({"account_id": account_id, "amount": U128(amount)}));
        Promise::new(self.fusd_id.clone())
            .function_call(
                "ft_transfer".to_string(),
                json!({"receiver_id": account_id, "amount": U128(amount), "memo": "flatrate refund"})
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(1),
                GAS_FT_TRANSFER,
            )
            .then(Self::ext(env::current_account_id()).with_static_gas(GAS_ON_PAYOUT).on_refund(account_id, U128(amount)))
            .detach();
        U128(amount)
    }

    /// Restores the refund balance if the fUSD transfer failed.
    #[private]
    pub fn on_refund(&mut self, account_id: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>) -> bool {
        if result.is_ok() {
            return true;
        }
        let cur = self.refunds.get(&account_id).copied().unwrap_or(0);
        self.refunds.insert(account_id.clone(), cur + amount.0);
        emit("refund_failed", json!({"account_id": account_id, "amount": amount}));
        false
    }

    pub fn refund_balance_of(&self, account_id: AccountId) -> U128 {
        U128(self.refunds.get(&account_id).copied().unwrap_or(0))
    }

    /// The commitment hash (hex) for these bid fields. Lets clients check their own hashing.
    pub fn bid_hash(&self, market_id: U64, side: Side, qty: U64, limit_cents: u8, salt: String) -> String {
        hex_encode(&bid_commitment(market_id.0, side, qty.0, limit_cents, &salt))
    }

    pub fn get_bids(&self, market_id: U64, from_index: Option<u32>, limit: Option<u32>) -> Vec<BidView> {
        let m = self.market(market_id.0);
        let from = from_index.unwrap_or(0);
        let limit = limit.unwrap_or(50).min(200);
        (from..m.bid_count)
            .take(limit as usize)
            .filter_map(|i| {
                self.bids.get(&(market_id.0, i)).map(|b| BidView {
                    index: i,
                    bidder: b.bidder.clone(),
                    hash: hex_encode(&b.hash),
                    escrow: U128(b.escrow),
                    committed_at: U64(b.committed_at),
                    revealed: b.revealed.clone(),
                    filled_qty: U64(b.filled_qty),
                    cost: U128(b.cost),
                })
            })
            .collect()
    }

    // ---------- claims ledger ----------

    /// fUSD `ft_transfer_call` receiver. `msg`: `{"action":"mint_pair","market_id":"<id>"}`.
    pub fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        require!(env::predecessor_account_id() == self.fusd_id, "ERR_WRONG_TOKEN");
        let parsed: TransferMsg =
            serde_json::from_str(&msg).unwrap_or_else(|_| env::panic_str("ERR_BAD_MSG"));
        match parsed {
            TransferMsg::MintPair { market_id } => self.internal_mint_pair(market_id.0, &sender_id, amount.0),
            TransferMsg::CommitBid { market_id, hash } => {
                self.internal_commit_bid(market_id.0, &sender_id, amount.0, &hash)
            }
        }
        PromiseOrValue::Value(U128(0))
    }

    fn internal_mint_pair(&mut self, market_id: u64, account_id: &AccountId, amount: u128) {
        let mut m = self.market(market_id);
        require!(m.status == MarketStatus::Open, "ERR_MARKET_NOT_OPEN");
        require!(amount > 0, "ERR_ZERO_AMOUNT");
        self.add_claim(market_id, Side::Long, account_id, amount);
        self.add_claim(market_id, Side::Short, account_id, amount);
        m.pairs += amount;
        m.collateral += amount;
        self.save(m);
        emit("mint_pair", json!({"market_id": U64(market_id), "account_id": account_id, "amount": U128(amount)}));
    }

    /// Burn `amount` Long plus `amount` Short and get `amount` fUSD back. Until settlement.
    pub fn redeem_pair(&mut self, market_id: U64, amount: U128) -> U128 {
        let account_id = env::predecessor_account_id();
        let mut m = self.market(market_id.0);
        require!(m.status == MarketStatus::Open, "ERR_MARKET_NOT_OPEN");
        require!(amount.0 > 0, "ERR_ZERO_AMOUNT");
        self.sub_claim(market_id.0, Side::Long, &account_id, amount.0);
        self.sub_claim(market_id.0, Side::Short, &account_id, amount.0);
        m.pairs -= amount.0;
        m.collateral -= amount.0;
        self.save(m);
        emit("redeem_pair", json!({"market_id": market_id, "account_id": account_id, "amount": amount}));
        self.pay(account_id, market_id.0, amount.0, amount.0, amount.0, amount.0);
        amount
    }

    /// Move claims to another account.
    pub fn claim_transfer(&mut self, receiver_id: AccountId, market_id: U64, side: Side, amount: U128) {
        let sender = env::predecessor_account_id();
        require!(sender != receiver_id, "ERR_SELF_TRANSFER");
        require!(amount.0 > 0, "ERR_ZERO_AMOUNT");
        self.market(market_id.0);
        self.sub_claim(market_id.0, side, &sender, amount.0);
        self.add_claim(market_id.0, side, &receiver_id, amount.0);
        emit(
            "claim_transfer",
            json!({"market_id": market_id, "side": side, "from": sender, "to": receiver_id, "amount": amount}),
        );
    }

    pub fn claim_balance_of(&self, account_id: AccountId, market_id: U64, side: Side) -> U128 {
        U128(self.claim(market_id.0, side, &account_id))
    }

    // ---------- observations through OutLayer ----------

    /// Anyone, during the market period. Attach the OutLayer deposit (see
    /// `estimate_execution_cost` on the OutLayer contract) and 300 TGas.
    #[payable]
    pub fn request_observation(&mut self, market_id: U64) -> Promise {
        let m = self.market(market_id.0);
        require!(m.status == MarketStatus::Open, "ERR_MARKET_NOT_OPEN");
        let now = env::block_timestamp();
        require!(now >= m.period_start && now < m.period_end, "ERR_OUTSIDE_PERIOD");
        let deposit = env::attached_deposit();
        require!(!deposit.is_zero(), "ERR_NO_DEPOSIT");
        let caller = env::predecessor_account_id();
        let pending_id = self.next_pending_id;
        self.next_pending_id += 1;
        self.pending.insert(
            pending_id,
            PendingObservation { market_id: m.id, caller: caller.clone(), deposit, requested_at: now },
        );
        emit("observation_requested", json!({"market_id": market_id, "pending_id": U64(pending_id), "caller": caller}));

        // request_execution(source, resource_limits, input_data, secrets_ref, response_format,
        //                   payer_account_id, params): near-outlayer contract/src/execution.rs:35-45
        let args = json!({
            "source": {"Project": {"project_id": self.oracle_project_id, "version_key": m.oracle_version_key}},
            "resource_limits": {
                "max_instructions": self.resource_limits.max_instructions.0,
                "max_memory_mb": self.resource_limits.max_memory_mb,
                "max_execution_seconds": self.resource_limits.max_execution_seconds.0,
            },
            "input_data": m.oracle_input,
            "secrets_ref": null,
            "response_format": "Json",
            "payer_account_id": caller,
            "params": null,
        });
        Promise::new(self.outlayer_id.clone())
            .function_call_weight(
                "request_execution".to_string(),
                args.to_string().into_bytes(),
                deposit,
                GAS_OUTLAYER_STATIC,
                GasWeight(1),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_ON_OBSERVATION)
                    .with_unused_gas_weight(0)
                    .on_observation(market_id, U64(pending_id)),
            )
    }

    /// Callback for `request_observation`. Only this contract's own promise chain can record
    /// a price: an id that was already used fails with `ERR_UNKNOWN_REQUEST`, any caller other
    /// than this contract fails with `ERR_NOT_OUTLAYER`, and an id never issued fails with
    /// `ERR_UNKNOWN_REQUEST`. See BLOCKERS.md B1.
    pub fn on_observation(&mut self, market_id: U64, pending_id: U64) -> bool {
        let id = pending_id.0;
        if id < self.next_pending_id && !self.pending.contains_key(&id) {
            env::panic_str("ERR_UNKNOWN_REQUEST");
        }
        if env::predecessor_account_id() != env::current_account_id() {
            env::panic_str("ERR_NOT_OUTLAYER");
        }
        let p = self.pending.remove(&id).unwrap_or_else(|| env::panic_str("ERR_UNKNOWN_REQUEST"));
        require!(p.market_id == market_id.0, "ERR_UNKNOWN_REQUEST");
        require!(env::promise_results_count() == 1, "ERR_UNKNOWN_REQUEST");

        let raw = match env::promise_result_checked(0, MAX_ORACLE_OUTPUT) {
            Ok(bytes) => bytes,
            Err(PromiseError::Failed) => {
                // OutLayer refused the call at the door, so the deposit came back to us.
                Promise::new(p.caller.clone()).transfer(p.deposit).detach();
                return self.reject(market_id.0, id, "outlayer_call_failed");
            }
            Err(PromiseError::TooLong(_)) => return self.reject(market_id.0, id, "output_too_long"),
            Err(_) => return self.reject(market_id.0, id, "outlayer_result_error"),
        };
        let out: Option<OracleOutput> = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => return self.reject(market_id.0, id, "bad_output"),
        };
        let Some(out) = out else {
            // Failed run or timeout: OutLayer returns null and refunds the payer itself.
            return self.reject(market_id.0, id, "oracle_no_output");
        };

        let mut m = self.market(market_id.0);
        if m.status != MarketStatus::Open {
            return self.reject(market_id.0, id, "market_not_open");
        }
        if out.index != m.index.as_str() {
            return self.reject(market_id.0, id, "wrong_index");
        }
        let ok: Vec<u64> = out.sources.iter().filter_map(|s| s.price_micros).collect();
        if ok.len() < MIN_SOURCES_OK as usize {
            return self.reject(market_id.0, id, "too_few_sources");
        }
        let Some(price) = out.price_micros else {
            return self.reject(market_id.0, id, "no_price");
        };
        if price == 0 || price as u128 >= 10 * m.cap_micros as u128 {
            return self.reject(market_id.0, id, "price_out_of_range");
        }
        if math::median(&ok) != Some(price) {
            return self.reject(market_id.0, id, "median_mismatch");
        }
        let obs = Observation {
            price_micros: price,
            sources_ok: ok.len().min(u8::MAX as usize) as u8,
            at: env::block_timestamp(),
            outlayer_request: out.request_id.unwrap_or_default(),
            pending_id: id,
        };
        emit(
            "observation_recorded",
            json!({"market_id": market_id, "pending_id": pending_id, "price_micros": U64(price),
                   "sources_ok": obs.sources_ok, "outlayer_request": obs.outlayer_request}),
        );
        m.observations.push(obs);
        self.save(m);
        true
    }

    fn reject(&mut self, market_id: u64, pending_id: u64, reason: &str) -> bool {
        emit(
            "observation_rejected",
            json!({"market_id": U64(market_id), "pending_id": U64(pending_id), "reason": reason}),
        );
        false
    }

    // ---------- settlement ----------

    /// Anyone, after `period_end`. Settles on the mean of the observations if there are at
    /// least `min_observations`; otherwise refuses until `void_after`, then voids. Final.
    pub fn settle(&mut self, market_id: U64) -> MarketStatus {
        let mut m = self.market(market_id.0);
        match m.status {
            MarketStatus::Settled { .. } | MarketStatus::Void => env::panic_str("ERR_ALREADY_SETTLED"),
            MarketStatus::Auction => env::panic_str("ERR_AUCTION_NOT_CLEARED"),
            MarketStatus::Open => {}
        }
        let now = env::block_timestamp();
        require!(now >= m.period_end, "ERR_PERIOD_NOT_ENDED");
        if m.observations.len() >= m.min_observations as usize {
            let prices: Vec<u64> = m.observations.iter().map(|o| o.price_micros).collect();
            let avg = math::mean(&prices).unwrap();
            let p = math::payoff(avg, m.floor_micros, m.cap_micros);
            m.status = MarketStatus::Settled { long_payout_micros: p.long_micros };
            emit(
                "market_settled",
                json!({"market_id": market_id, "avg_micros": U64(avg), "long_payout_micros": U64(p.long_micros),
                       "short_payout_micros": U64(p.short_micros), "observations": m.observations.len()}),
            );
        } else if now >= m.void_after {
            m.status = MarketStatus::Void;
            emit("market_void", json!({"market_id": market_id, "observations": m.observations.len()}));
        } else {
            env::panic_str("ERR_TOO_FEW_OBSERVATIONS");
        }
        let status = m.status.clone();
        self.save(m);
        status
    }

    /// After settlement: pays `balance x payout` for each side held, then zeroes the balances.
    pub fn redeem(&mut self, market_id: U64) -> U128 {
        let account_id = env::predecessor_account_id();
        let mut m = self.market(market_id.0);
        let p = match m.status {
            MarketStatus::Settled { long_payout_micros } => math::Payoff {
                long_micros: long_payout_micros,
                short_micros: math::PAIR_MICROS - long_payout_micros,
            },
            MarketStatus::Void => math::VOID_PAYOFF,
            _ => env::panic_str("ERR_NOT_SETTLED"),
        };
        let long = self.claim(market_id.0, Side::Long, &account_id);
        let short = self.claim(market_id.0, Side::Short, &account_id);
        require!(long > 0 || short > 0, "ERR_NOTHING_TO_REDEEM");
        self.set_claim(market_id.0, Side::Long, &account_id, 0);
        self.set_claim(market_id.0, Side::Short, &account_id, 0);
        let amount = math::redeem_amount(long, p.long_micros) + math::redeem_amount(short, p.short_micros);
        m.collateral -= amount;
        self.save(m);
        emit(
            "redeem",
            json!({"market_id": market_id, "account_id": account_id, "long": U128(long), "short": U128(short), "amount": U128(amount)}),
        );
        if amount > 0 {
            self.pay(account_id, market_id.0, amount, long, short, 0);
        }
        U128(amount)
    }

    /// Restores balances if an fUSD payout failed (for example, receiver not registered).
    #[private]
    pub fn on_payout(
        &mut self,
        account_id: AccountId,
        market_id: U64,
        amount: U128,
        restore_long: U128,
        restore_short: U128,
        restore_pairs: U128,
        #[callback_result] result: Result<(), PromiseError>,
    ) -> bool {
        if result.is_ok() {
            return true;
        }
        let mut m = self.market(market_id.0);
        m.collateral += amount.0;
        m.pairs += restore_pairs.0;
        self.save(m);
        self.add_claim(market_id.0, Side::Long, &account_id, restore_long.0);
        self.add_claim(market_id.0, Side::Short, &account_id, restore_short.0);
        emit("payout_failed", json!({"market_id": market_id, "account_id": account_id, "amount": amount}));
        false
    }

    // ---------- views ----------

    pub fn get_config(&self) -> ConfigView {
        ConfigView {
            owner_id: self.owner_id.clone(),
            fusd_id: self.fusd_id.clone(),
            outlayer_id: self.outlayer_id.clone(),
            oracle_project_id: self.oracle_project_id.clone(),
            oracle_version_key: self.oracle_version_key.clone(),
            oracle_sources: self.oracle_sources.clone(),
            resource_limits: self.resource_limits.clone(),
            next_pending_id: U64(self.next_pending_id),
            market_count: U64(self.markets.len() as u64),
        }
    }

    pub fn get_market(&self, market_id: U64) -> Option<MarketView> {
        self.markets.get(market_id.0 as u32).map(MarketView::from)
    }

    pub fn list_markets(&self, from_index: Option<U64>, limit: Option<u32>) -> Vec<MarketView> {
        let from = from_index.map(|v| v.0).unwrap_or(0) as u32;
        let limit = limit.unwrap_or(50).min(100);
        (from..self.markets.len())
            .take(limit as usize)
            .filter_map(|i| self.markets.get(i).map(MarketView::from))
            .collect()
    }

    pub fn is_pending(&self, pending_id: U64) -> bool {
        self.pending.contains_key(&pending_id.0)
    }

    // ---------- internal ----------

    fn market(&self, id: u64) -> Market {
        self.markets
            .get(u32::try_from(id).unwrap_or(u32::MAX))
            .cloned()
            .unwrap_or_else(|| env::panic_str("ERR_NO_MARKET"))
    }

    fn save(&mut self, m: Market) {
        let i = m.id as u32;
        self.markets.replace(i, m);
    }

    fn key(market_id: u64, side: Side, account_id: &AccountId) -> ClaimKey {
        ClaimKey { market_id, side, account_id: account_id.clone() }
    }

    fn claim(&self, market_id: u64, side: Side, account_id: &AccountId) -> u128 {
        self.claims.get(&Self::key(market_id, side, account_id)).copied().unwrap_or(0)
    }

    fn set_claim(&mut self, market_id: u64, side: Side, account_id: &AccountId, v: u128) {
        let k = Self::key(market_id, side, account_id);
        if v == 0 {
            self.claims.remove(&k);
        } else {
            self.claims.insert(k, v);
        }
    }

    fn add_claim(&mut self, market_id: u64, side: Side, account_id: &AccountId, v: u128) {
        if v == 0 {
            return;
        }
        let cur = self.claim(market_id, side, account_id);
        self.set_claim(market_id, side, account_id, cur + v);
    }

    fn sub_claim(&mut self, market_id: u64, side: Side, account_id: &AccountId, v: u128) {
        let cur = self.claim(market_id, side, account_id);
        require!(cur >= v, "ERR_INSUFFICIENT_CLAIMS");
        self.set_claim(market_id, side, account_id, cur - v);
    }

    /// Sends `amount` fUSD; restores the given balances if the transfer fails.
    fn pay(
        &self,
        account_id: AccountId,
        market_id: u64,
        amount: u128,
        restore_long: u128,
        restore_short: u128,
        restore_pairs: u128,
    ) {
        Promise::new(self.fusd_id.clone())
            .function_call(
                "ft_transfer".to_string(),
                json!({"receiver_id": account_id, "amount": U128(amount), "memo": "flatrate"})
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(1),
                GAS_FT_TRANSFER,
            )
            .then(Self::ext(env::current_account_id()).with_static_gas(GAS_ON_PAYOUT).on_payout(
                account_id,
                U64(market_id),
                U128(amount),
                U128(restore_long),
                U128(restore_short),
                U128(restore_pairs),
            ))
            .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_rejects_bad_input() {
        let h = [0xabu8; 32];
        let s = hex_encode(&h);
        assert_eq!(s.len(), 64);
        assert_eq!(hex_decode_32(&s), Some(h));
        assert_eq!(hex_decode_32(&s.to_uppercase()), Some(h));
        assert_eq!(hex_decode_32(&s[..62]), None);
        assert_eq!(hex_decode_32(&format!("zz{}", &s[2..])), None);
    }

    #[test]
    fn commitment_depends_on_every_field() {
        let base = bid_commitment(0, Side::Long, 10, 60, "salt");
        assert_eq!(base, bid_commitment(0, Side::Long, 10, 60, "salt"));
        assert_ne!(base, bid_commitment(1, Side::Long, 10, 60, "salt"));
        assert_ne!(base, bid_commitment(0, Side::Short, 10, 60, "salt"));
        assert_ne!(base, bid_commitment(0, Side::Long, 11, 60, "salt"));
        assert_ne!(base, bid_commitment(0, Side::Long, 10, 61, "salt"));
        assert_ne!(base, bid_commitment(0, Side::Long, 10, 60, "salt2"));
    }
}
