//! Flatrate: GPU price claims settled on an OutLayer-attested median.
//!
//! Markets made from fixed templates, a sealed commit-reveal opening auction, a claims ledger,
//! one price observation per time slot through the OutLayer callback, settlement on the mean
//! of the filled slots, public market rolling, and Cover Note NFTs.
//!
//! The OutLayer fee for an observation is paid from the contract's own balance (the
//! observation budget, which anyone can top up), so keepers need no deposit and can run on a
//! function-call key.

use flatrate_shared as math;
use near_contract_standards::non_fungible_token::core::NonFungibleTokenCore;
use near_contract_standards::non_fungible_token::events::{NftBurn, NftMint};
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::{
    NonFungibleToken, NonFungibleTokenEnumeration, NonFungibleTokenResolver, Token, TokenId,
};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::{self, json};
use near_sdk::store::LookupMap;
use near_sdk::{env, near, require, AccountId, Gas, GasWeight, NearToken, PanicOnDefault, Promise, PromiseError, PromiseOrValue};

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
/// NEAR kept back from the observation budget, on top of storage.
const BUDGET_RESERVE: NearToken = NearToken::from_millinear(500);
/// Market ids start here and Cover Note ids at `FIRST_NOTE_ID`, so they never repeat the ids
/// used by the contract state this one replaced (markets 0-5, notes 0-2).
const FIRST_MARKET_ID: u64 = 10;
const FIRST_NOTE_ID: u64 = 1000;
/// 10 cents in micro-USD: rolled bands are rounded down to this step.
const TEN_CENTS: u64 = 100_000;

// Storage prefixes. Byte strings distinct from the previous state's enum prefixes, so a reset
// in place never reads old data.
const P_MARKETS: &[u8] = b"m2";
const P_LATEST: &[u8] = b"l2";
const P_OBS: &[u8] = b"o2";
const P_SLOT_PENDING: &[u8] = b"s2";
const P_CLAIMS: &[u8] = b"c2";
const P_PENDING: &[u8] = b"p2";
const P_BIDS: &[u8] = b"b2";
const P_BID_INDEX: &[u8] = b"i2";
const P_REFUNDS: &[u8] = b"r2";
const P_NFT_OWNERS: &[u8] = b"n2o";
const P_NFT_META: &[u8] = b"n2m";
const P_NFT_ENUM: &[u8] = b"n2e";
const P_NOTES: &[u8] = b"n2n";

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::upper_case_acronyms)]
pub enum Template {
    DEMO,
    WEEKLY,
}

/// Lengths for markets made from a template. Stored at init; there is no setter.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct TemplateConfig {
    pub commit_ns: U64,
    pub reveal_ns: U64,
    pub period_ns: U64,
    pub slot_ns: U64,
    pub min_filled_slots: u32,
    pub void_after_ns: U64,
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
    /// This contract's pending id for the request.
    pub pending_id: u64,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct Market {
    pub id: u64,
    pub template: Template,
    pub index: IndexId,
    pub floor_micros: u64,
    pub cap_micros: u64,
    pub auction_commit_end: u64,
    pub auction_reveal_end: u64,
    pub period_start: u64,
    pub period_end: u64,
    pub slot_ns: u64,
    pub min_filled_slots: u32,
    pub void_after: u64,
    pub status: MarketStatus,
    pub filled_slots: u32,
    pub settled_average_micros: Option<u64>,
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
    pub bid_count: u32,
    /// Account that created (owner, first market) or rolled the market.
    pub created_by: AccountId,
}

impl Market {
    pub fn num_slots(&self) -> u32 {
        (self.period_end - self.period_start).div_ceil(self.slot_ns) as u32
    }
    /// The slot `now` falls in, if `now` is inside the period.
    pub fn slot_at(&self, now: u64) -> Option<u32> {
        (now >= self.period_start && now < self.period_end).then(|| ((now - self.period_start) / self.slot_ns) as u32)
    }
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
pub struct PendingObservation {
    pub market_id: u64,
    pub slot: u32,
    pub caller: AccountId,
    pub requested_at: u64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct ObservationView {
    pub slot: u32,
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
    pub template: Template,
    pub index: IndexId,
    pub floor_micros: U64,
    pub cap_micros: U64,
    pub auction_commit_end: U64,
    pub auction_reveal_end: U64,
    pub period_start: U64,
    pub period_end: U64,
    pub slot_ns: U64,
    pub num_slots: u32,
    pub min_filled_slots: u32,
    pub void_after: U64,
    pub status: MarketStatus,
    pub filled_slots: u32,
    pub settled_average_micros: Option<U64>,
    pub observations: Vec<ObservationView>,
    pub oracle_version_key: String,
    pub oracle_input: String,
    pub opening_price_cents: Option<u8>,
    pub pairs: U128,
    pub collateral: U128,
    pub bid_count: u32,
    pub created_by: AccountId,
}

#[near(serializers = [json])]
pub struct SlotInfo {
    pub market_id: U64,
    pub num_slots: u32,
    /// Slot that `now` falls in (None outside the period).
    pub current_slot: Option<u32>,
    pub current_filled: bool,
    pub current_pending: bool,
    pub filled_slots: u32,
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
    pub demo_template: TemplateConfig,
    pub weekly_template: TemplateConfig,
    pub observation_deposit: U128,
    pub observation_budget: U128,
    pub next_pending_id: U64,
    pub next_market_id: U64,
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

/// Claims held inside a Cover Note NFT. They pay whoever owns the token at redemption.
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct Note {
    pub market_id: U64,
    pub side: Side,
    /// Claim units (6 decimals).
    pub amount: U128,
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

/// `YYYY-MM-DD` (UTC) for a ns timestamp (Howard Hinnant's civil-from-days).
fn utc_date(ns: u64) -> String {
    let z = (ns / 1_000_000_000 / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Claim units as whole claims, without trailing zeros: 100_000_000 -> "100", 1_500_000 -> "1.5".
fn claims_str(units: u128) -> String {
    let whole = units / CLAIM_UNITS;
    let frac = units % CLAIM_UNITS;
    if frac == 0 {
        whole.to_string()
    } else {
        let f = format!("{frac:06}");
        format!("{whole}.{}", f.trim_end_matches('0'))
    }
}

fn usd_str(micros: u64) -> String {
    format!("${}.{:02}", micros / 1_000_000, (micros % 1_000_000) / 10_000)
}

/// Band for a rolled market: centre x 0.6 and centre x 1.4, both rounded down to 10 cents.
/// `None` if the result is not a valid band (floor 0 or cap <= floor).
pub fn band_from_average(centre_micros: u64) -> Option<(u64, u64)> {
    let floor = (centre_micros as u128 * 6 / 10) as u64 / TEN_CENTS * TEN_CENTS;
    let cap = (centre_micros as u128 * 14 / 10) as u64 / TEN_CENTS * TEN_CENTS;
    (floor > 0 && cap > floor).then_some((floor, cap))
}

fn emit(event: &str, data: serde_json::Value) {
    env::log_str(&format!(
        "EVENT_JSON:{}",
        json!({"standard": "flatrate", "version": "2.0.0", "event": event, "data": [data]})
    ));
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    fusd_id: AccountId,
    outlayer_id: AccountId,
    oracle_project_id: String,
    /// Pinned for every market. Fixed at init: there is no setter.
    oracle_version_key: String,
    oracle_sources: Vec<String>,
    resource_limits: ResourceLimits,
    demo_template: TemplateConfig,
    weekly_template: TemplateConfig,
    /// yoctoNEAR attached to each OutLayer request; OutLayer refunds the unused part to us.
    observation_deposit: NearToken,
    markets: LookupMap<u64, Market>,
    next_market_id: u64,
    /// Latest market id per template.
    latest: LookupMap<Template, u64>,
    /// (market_id, slot) -> the observation recorded for that slot.
    observations: LookupMap<(u64, u32), Observation>,
    /// (market_id, slot) -> pending id of the request in flight for that slot.
    slot_pending: LookupMap<(u64, u32), u64>,
    claims: LookupMap<ClaimKey, u128>,
    pending: LookupMap<u64, PendingObservation>,
    next_pending_id: u64,
    /// (market_id, bid index) -> bid. Index order is commit order.
    bids: LookupMap<(u64, u32), Bid>,
    /// (market_id, bidder, hash) -> bid index, for `reveal_bid`.
    bid_index: LookupMap<(u64, AccountId, [u8; 32]), u32>,
    /// fUSD owed back to an account (unused auction escrow), withdrawn with `withdraw_refund`.
    refunds: LookupMap<AccountId, u128>,
    /// Cover Notes (NEP-171 core, NEP-177 metadata, NEP-181 enumeration).
    tokens: NonFungibleToken,
    notes: LookupMap<TokenId, Note>,
    next_note_id: u64,
}

#[near]
impl Contract {
    #[init]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner_id: AccountId,
        fusd_id: AccountId,
        outlayer_id: AccountId,
        oracle_project_id: String,
        oracle_version_key: String,
        oracle_sources: Vec<String>,
        resource_limits: ResourceLimits,
        demo_template: TemplateConfig,
        weekly_template: TemplateConfig,
        observation_deposit: U128,
    ) -> Self {
        require!(!oracle_version_key.is_empty(), "ERR_EMPTY_VERSION");
        require!(oracle_sources.len() >= MIN_SOURCES_OK as usize, "ERR_TOO_FEW_SOURCES");
        require!(observation_deposit.0 > 0, "ERR_ZERO_DEPOSIT");
        for t in [&demo_template, &weekly_template] {
            require!(t.slot_ns.0 > 0 && t.period_ns.0 >= t.slot_ns.0 && t.commit_ns.0 > 0, "ERR_BAD_TEMPLATE");
            require!(t.min_filled_slots >= 1, "ERR_BAD_TEMPLATE");
            require!((t.min_filled_slots as u64) <= t.period_ns.0.div_ceil(t.slot_ns.0), "ERR_BAD_TEMPLATE");
        }
        Self {
            tokens: NonFungibleToken::new(P_NFT_OWNERS.to_vec(), owner_id.clone(), Some(P_NFT_META.to_vec()), Some(P_NFT_ENUM.to_vec()), None::<Vec<u8>>),
            notes: LookupMap::new(P_NOTES),
            next_note_id: FIRST_NOTE_ID,
            owner_id,
            fusd_id,
            outlayer_id,
            oracle_project_id,
            oracle_version_key,
            oracle_sources,
            resource_limits,
            demo_template,
            weekly_template,
            observation_deposit: NearToken::from_yoctonear(observation_deposit.0),
            markets: LookupMap::new(P_MARKETS),
            next_market_id: FIRST_MARKET_ID,
            latest: LookupMap::new(P_LATEST),
            observations: LookupMap::new(P_OBS),
            slot_pending: LookupMap::new(P_SLOT_PENDING),
            claims: LookupMap::new(P_CLAIMS),
            pending: LookupMap::new(P_PENDING),
            next_pending_id: 0,
            bids: LookupMap::new(P_BIDS),
            bid_index: LookupMap::new(P_BID_INDEX),
            refunds: LookupMap::new(P_REFUNDS),
        }
    }

    /// Fresh start on an account that already holds an older Flatrate state: same arguments as
    /// `new`, old state ignored (it stays in storage under other prefixes and is never read).
    /// Only the contract account can call it, as the init call of a deploy.
    #[private]
    #[init(ignore_state)]
    #[allow(clippy::too_many_arguments)]
    pub fn reset(
        owner_id: AccountId,
        fusd_id: AccountId,
        outlayer_id: AccountId,
        oracle_project_id: String,
        oracle_version_key: String,
        oracle_sources: Vec<String>,
        resource_limits: ResourceLimits,
        demo_template: TemplateConfig,
        weekly_template: TemplateConfig,
        observation_deposit: U128,
    ) -> Self {
        Self::new(
            owner_id, fusd_id, outlayer_id, oracle_project_id, oracle_version_key, oracle_sources, resource_limits,
            demo_template, weekly_template, observation_deposit,
        )
    }

    // ---------- markets ----------

    fn template(&self, t: Template) -> &TemplateConfig {
        match t {
            Template::DEMO => &self.demo_template,
            Template::WEEKLY => &self.weekly_template,
        }
    }

    fn internal_new_market(&mut self, template: Template, floor: u64, cap: u64) -> u64 {
        require!(floor > 0 && cap > floor, "ERR_BAD_BOUNDS");
        let t = self.template(template).clone();
        let now = env::block_timestamp();
        let commit_end = now + t.commit_ns.0;
        let reveal_end = commit_end + t.reveal_ns.0;
        let period_end = reveal_end + t.period_ns.0;
        let id = self.next_market_id;
        self.next_market_id += 1;
        let index = IndexId::H100_HOURLY;
        let m = Market {
            id,
            template,
            index,
            floor_micros: floor,
            cap_micros: cap,
            auction_commit_end: commit_end,
            auction_reveal_end: reveal_end,
            period_start: reveal_end,
            period_end,
            slot_ns: t.slot_ns.0,
            min_filled_slots: t.min_filled_slots,
            void_after: period_end + t.void_after_ns.0,
            status: MarketStatus::Auction,
            filled_slots: 0,
            settled_average_micros: None,
            oracle_version_key: self.oracle_version_key.clone(),
            oracle_input: json!({"index": index.as_str(), "sources": self.oracle_sources}).to_string(),
            opening_price_cents: None,
            pairs: 0,
            collateral: 0,
            bid_count: 0,
            created_by: env::predecessor_account_id(),
        };
        self.markets.insert(id, m);
        self.latest.insert(template, id);
        emit(
            "market_created",
            json!({"market_id": U64(id), "template": template, "floor_micros": U64(floor), "cap_micros": U64(cap),
                   "created_by": env::predecessor_account_id()}),
        );
        id
    }

    /// Owner only, once per template: opens the first market of `template` with a band set by
    /// hand. Every later market of that template comes from `roll_market`.
    pub fn create_market(&mut self, template: Template, floor_micros: U64, cap_micros: U64) -> U64 {
        require!(env::predecessor_account_id() == self.owner_id, "ERR_NOT_OWNER");
        require!(!self.latest.contains_key(&template), "ERR_ALREADY_CREATED");
        U64(self.internal_new_market(template, floor_micros.0, cap_micros.0))
    }

    /// Anyone: opens the next market of `template` once the previous one is settled or void.
    /// The band is centred on the previous settled average (x0.6 / x1.4, rounded down to 10
    /// cents); after a void market the previous band is reused. Storage is paid by the contract.
    pub fn roll_market(&mut self, template: Template) -> U64 {
        let prev_id = self.latest.get(&template).copied().unwrap_or_else(|| env::panic_str("ERR_NO_PREVIOUS"));
        let prev = self.market(prev_id);
        let (floor, cap) = match prev.status {
            MarketStatus::Settled { .. } => prev
                .settled_average_micros
                .and_then(band_from_average)
                .unwrap_or((prev.floor_micros, prev.cap_micros)),
            MarketStatus::Void => (prev.floor_micros, prev.cap_micros),
            _ => env::panic_str("ERR_PREVIOUS_NOT_DONE"),
        };
        let id = self.internal_new_market(template, floor, cap);
        emit("market_rolled", json!({"market_id": U64(id), "previous": U64(prev_id), "template": template}));
        U64(id)
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

        let mut bids: Vec<Bid> = (0..m.bid_count).map(|i| self.bids.get(&(market_id.0, i)).cloned().unwrap()).collect();
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
                json!({"receiver_id": account_id, "amount": U128(amount), "memo": "flatrate refund"}).to_string().into_bytes(),
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

    /// fUSD `ft_transfer_call` receiver. `msg`: `{"action":"mint_pair","market_id":"<id>"}` or
    /// `{"action":"commit_bid","market_id":"<id>","hash":"<64 hex>"}`.
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        require!(env::predecessor_account_id() == self.fusd_id, "ERR_WRONG_TOKEN");
        let parsed: TransferMsg = serde_json::from_str(&msg).unwrap_or_else(|_| env::panic_str("ERR_BAD_MSG"));
        match parsed {
            TransferMsg::MintPair { market_id } => self.internal_mint_pair(market_id.0, &sender_id, amount.0),
            TransferMsg::CommitBid { market_id, hash } => self.internal_commit_bid(market_id.0, &sender_id, amount.0, &hash),
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
        emit("claim_transfer", json!({"market_id": market_id, "side": side, "from": sender, "to": receiver_id, "amount": amount}));
    }

    pub fn claim_balance_of(&self, account_id: AccountId, market_id: U64, side: Side) -> U128 {
        U128(self.claim(market_id.0, side, &account_id))
    }

    // ---------- observations through OutLayer ----------

    /// NEAR available to pay OutLayer: the contract balance above storage and a reserve.
    fn budget(&self) -> u128 {
        let storage = env::storage_byte_cost().as_yoctonear() * env::storage_usage() as u128;
        env::account_balance().as_yoctonear().saturating_sub(storage + BUDGET_RESERVE.as_yoctonear())
    }

    /// Anyone: adds the attached NEAR to the observation budget.
    #[payable]
    pub fn fund_observations(&mut self) -> U128 {
        let d = env::attached_deposit();
        require!(!d.is_zero(), "ERR_NO_DEPOSIT");
        emit("observations_funded", json!({"by": env::predecessor_account_id(), "amount": U128(d.as_yoctonear())}));
        U128(self.budget())
    }

    /// Anyone, during the period, for the current slot if it has no price and no request in
    /// flight. The contract pays OutLayer from the observation budget (no deposit needed) and
    /// OutLayer refunds the unused part to the contract. Attach 300 TGas.
    pub fn request_observation(&mut self, market_id: U64) -> Promise {
        let m = self.market(market_id.0);
        require!(m.status == MarketStatus::Open, "ERR_MARKET_NOT_OPEN");
        let now = env::block_timestamp();
        let slot = m.slot_at(now).unwrap_or_else(|| env::panic_str("ERR_NOT_IN_PERIOD"));
        let key = (m.id, slot);
        require!(!self.observations.contains_key(&key) && !self.slot_pending.contains_key(&key), "ERR_SLOT_FILLED");
        require!(self.budget() >= self.observation_deposit.as_yoctonear(), "ERR_NO_BUDGET");
        let caller = env::predecessor_account_id();
        let pending_id = self.next_pending_id;
        self.next_pending_id += 1;
        self.pending.insert(pending_id, PendingObservation { market_id: m.id, slot, caller: caller.clone(), requested_at: now });
        self.slot_pending.insert(key, pending_id);
        emit(
            "observation_requested",
            json!({"market_id": market_id, "slot": slot, "pending_id": U64(pending_id), "caller": caller}),
        );

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
            "payer_account_id": env::current_account_id(),
            "params": null,
        });
        Promise::new(self.outlayer_id.clone())
            .function_call_weight(
                "request_execution".to_string(),
                args.to_string().into_bytes(),
                self.observation_deposit,
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
    /// `ERR_UNKNOWN_REQUEST` (BLOCKERS.md B1). The result goes into the slot recorded at request
    /// time, even if it arrives after that slot ended. A rejected result frees the slot.
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
        self.slot_pending.remove(&(p.market_id, p.slot));

        let raw = match env::promise_result_checked(0, MAX_ORACLE_OUTPUT) {
            Ok(bytes) => bytes,
            // OutLayer refused the call at the door; the deposit came back to this contract.
            Err(PromiseError::Failed) => return self.reject(&p, id, "outlayer_call_failed"),
            Err(PromiseError::TooLong(_)) => return self.reject(&p, id, "output_too_long"),
            Err(_) => return self.reject(&p, id, "outlayer_result_error"),
        };
        let out: Option<OracleOutput> = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => return self.reject(&p, id, "bad_output"),
        };
        // Failed run or timeout: OutLayer returns null and refunds the payer (this contract).
        let Some(out) = out else { return self.reject(&p, id, "oracle_no_output") };

        let mut m = self.market(p.market_id);
        if m.status != MarketStatus::Open {
            return self.reject(&p, id, "market_not_open");
        }
        if out.index != m.index.as_str() {
            return self.reject(&p, id, "wrong_index");
        }
        let ok: Vec<u64> = out.sources.iter().filter_map(|s| s.price_micros).collect();
        if ok.len() < MIN_SOURCES_OK as usize {
            return self.reject(&p, id, "too_few_sources");
        }
        let Some(price) = out.price_micros else { return self.reject(&p, id, "no_price") };
        if price == 0 || price as u128 >= 10 * m.cap_micros as u128 {
            return self.reject(&p, id, "price_out_of_range");
        }
        if math::median(&ok) != Some(price) {
            return self.reject(&p, id, "median_mismatch");
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
            json!({"market_id": market_id, "slot": p.slot, "pending_id": pending_id, "price_micros": U64(price),
                   "sources_ok": obs.sources_ok, "outlayer_request": obs.outlayer_request}),
        );
        self.observations.insert((m.id, p.slot), obs);
        m.filled_slots += 1;
        self.save(m);
        true
    }

    fn reject(&mut self, p: &PendingObservation, pending_id: u64, reason: &str) -> bool {
        emit(
            "observation_rejected",
            json!({"market_id": U64(p.market_id), "slot": p.slot, "pending_id": U64(pending_id), "reason": reason}),
        );
        false
    }

    // ---------- settlement ----------

    /// Anyone, after `period_end`. Settles on the mean of the filled slots if there are at least
    /// `min_filled_slots`; otherwise refuses until `void_after`, then voids (0.50 per side). Final.
    pub fn settle(&mut self, market_id: U64) -> MarketStatus {
        let mut m = self.market(market_id.0);
        match m.status {
            MarketStatus::Settled { .. } | MarketStatus::Void => env::panic_str("ERR_ALREADY_SETTLED"),
            MarketStatus::Auction => env::panic_str("ERR_AUCTION_NOT_CLEARED"),
            MarketStatus::Open => {}
        }
        let now = env::block_timestamp();
        require!(now >= m.period_end, "ERR_PERIOD_NOT_ENDED");
        if m.filled_slots >= m.min_filled_slots {
            let prices: Vec<u64> =
                (0..m.num_slots()).filter_map(|s| self.observations.get(&(m.id, s)).map(|o| o.price_micros)).collect();
            let avg = math::mean(&prices).unwrap();
            let p = math::payoff(avg, m.floor_micros, m.cap_micros);
            m.status = MarketStatus::Settled { long_payout_micros: p.long_micros };
            m.settled_average_micros = Some(avg);
            emit(
                "market_settled",
                json!({"market_id": market_id, "avg_micros": U64(avg), "long_payout_micros": U64(p.long_micros),
                       "short_payout_micros": U64(p.short_micros), "filled_slots": prices.len()}),
            );
        } else if now >= m.void_after {
            m.status = MarketStatus::Void;
            emit("market_void", json!({"market_id": market_id, "filled_slots": m.filled_slots}));
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
        let p = Self::payoff_of(&m);
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

    fn payoff_of(m: &Market) -> math::Payoff {
        match m.status {
            MarketStatus::Settled { long_payout_micros } => {
                math::Payoff { long_micros: long_payout_micros, short_micros: math::PAIR_MICROS - long_payout_micros }
            }
            MarketStatus::Void => math::VOID_PAYOFF,
            _ => env::panic_str("ERR_NOT_SETTLED"),
        }
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

    // ---------- Cover Notes (NEP-171) ----------

    /// Moves `amount` of the caller's claims into a new Cover Note NFT owned by the caller.
    /// While the market is open. The note pays whoever holds it after settlement.
    pub fn wrap_note(&mut self, market_id: U64, side: Side, amount: U128) -> Token {
        let owner = env::predecessor_account_id();
        let m = self.market(market_id.0);
        require!(m.status == MarketStatus::Open, "ERR_MARKET_NOT_OPEN");
        require!(amount.0 > 0, "ERR_ZERO_AMOUNT");
        self.sub_claim(market_id.0, side, &owner, amount.0);

        let token_id: TokenId = self.next_note_id.to_string();
        self.next_note_id += 1;
        let side_str = match side {
            Side::Long => "Long",
            Side::Short => "Short",
        };
        let amt = claims_str(amount.0);
        let metadata = TokenMetadata {
            title: Some(format!("Cover Note #{token_id}: {amt} {side_str}, H100, period ending {}", utc_date(m.period_end))),
            description: Some(format!(
                "Holds {amt} {side_str} claims of Flatrate market {} (H100 per GPU-hour, floor {}, cap {}). \
                 After settlement the holder redeems it for fUSD.",
                m.id,
                usd_str(m.floor_micros),
                usd_str(m.cap_micros)
            )),
            media: None,
            media_hash: None,
            copies: Some(1),
            issued_at: Some((env::block_timestamp() / 1_000_000).to_string()),
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: Some(
                json!({"market_id": U64(m.id), "side": side, "amount": amount, "index": m.index.as_str(),
                       "floor_micros": U64(m.floor_micros), "cap_micros": U64(m.cap_micros),
                       "period_end": U64(m.period_end)})
                .to_string(),
            ),
            reference: None,
            reference_hash: None,
        };
        let token = self.tokens.internal_mint_with_refund(token_id.clone(), owner.clone(), Some(metadata), None);
        self.notes.insert(token_id.clone(), Note { market_id, side, amount });
        NftMint { owner_id: &owner, token_ids: &[&token_id], memo: None }.emit();
        emit("note_wrapped", json!({"token_id": token_id, "owner": owner, "market_id": market_id, "side": side, "amount": amount}));
        token
    }

    /// Before settlement: the owner gets the claims back and the note is burned.
    pub fn unwrap_note(&mut self, token_id: TokenId) -> U128 {
        let (owner, note) = self.note_for_owner(&token_id);
        let m = self.market(note.market_id.0);
        require!(m.status == MarketStatus::Open, "ERR_ALREADY_SETTLED");
        self.burn_note(&token_id, &owner);
        self.add_claim(note.market_id.0, note.side, &owner, note.amount.0);
        emit("note_unwrapped", json!({"token_id": token_id, "owner": owner, "amount": note.amount}));
        note.amount
    }

    /// After settlement: pays the note's claims to the current owner in fUSD and burns it.
    pub fn redeem_note(&mut self, token_id: TokenId) -> U128 {
        let (owner, note) = self.note_for_owner(&token_id);
        let mut m = self.market(note.market_id.0);
        let p = Self::payoff_of(&m);
        let per_claim = match note.side {
            Side::Long => p.long_micros,
            Side::Short => p.short_micros,
        };
        let amount = math::redeem_amount(note.amount.0, per_claim);
        self.burn_note(&token_id, &owner);
        m.collateral -= amount;
        self.save(m);
        emit("note_redeemed", json!({"token_id": token_id, "owner": owner, "amount": U128(amount)}));
        if amount > 0 {
            // If the transfer fails, the claims go back to the owner's ledger balance.
            let (rl, rs) = match note.side {
                Side::Long => (note.amount.0, 0),
                Side::Short => (0, note.amount.0),
            };
            self.pay(owner, note.market_id.0, amount, rl, rs, 0);
        }
        U128(amount)
    }

    /// The note behind a Cover Note token.
    pub fn note_info(&self, token_id: TokenId) -> Option<Note> {
        self.notes.get(&token_id).cloned()
    }

    /// Owner check first, so a caller who does not own the note always sees `ERR_NOT_OWNER`.
    fn note_for_owner(&self, token_id: &TokenId) -> (AccountId, Note) {
        let owner = self.tokens.owner_by_id.get(token_id).unwrap_or_else(|| env::panic_str("ERR_NO_NOTE"));
        require!(env::predecessor_account_id() == owner, "ERR_NOT_OWNER");
        let note = self.notes.get(token_id).cloned().unwrap_or_else(|| env::panic_str("ERR_NO_NOTE"));
        (owner, note)
    }

    fn burn_note(&mut self, token_id: &TokenId, owner: &AccountId) {
        self.tokens.owner_by_id.remove(token_id);
        if let Some(meta) = self.tokens.token_metadata_by_id.as_mut() {
            meta.remove(token_id);
        }
        if let Some(per_owner) = self.tokens.tokens_per_owner.as_mut() {
            if let Some(mut set) = per_owner.get(owner) {
                set.remove(token_id);
                if set.is_empty() {
                    per_owner.remove(owner);
                } else {
                    per_owner.insert(owner, &set);
                }
            }
        }
        self.notes.remove(token_id);
        NftBurn { owner_id: owner, token_ids: &[token_id], authorized_id: None, memo: None }.emit();
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
            demo_template: self.demo_template.clone(),
            weekly_template: self.weekly_template.clone(),
            observation_deposit: U128(self.observation_deposit.as_yoctonear()),
            observation_budget: U128(self.budget()),
            next_pending_id: U64(self.next_pending_id),
            next_market_id: U64(self.next_market_id),
        }
    }

    pub fn get_market(&self, market_id: U64) -> Option<MarketView> {
        self.markets.get(&market_id.0).map(|m| self.view_of(m))
    }

    /// Markets by id, starting at `from_index` (default: the first market id).
    pub fn list_markets(&self, from_index: Option<U64>, limit: Option<u32>) -> Vec<MarketView> {
        let from = from_index.map(|v| v.0).unwrap_or(FIRST_MARKET_ID).max(FIRST_MARKET_ID);
        let limit = limit.unwrap_or(50).min(100) as u64;
        (from..self.next_market_id).take(limit as usize).filter_map(|i| self.markets.get(&i).map(|m| self.view_of(m))).collect()
    }

    pub fn latest_market(&self, template: Template) -> Option<U64> {
        self.latest.get(&template).map(|id| U64(*id))
    }

    pub fn slot_info(&self, market_id: U64) -> SlotInfo {
        let m = self.market(market_id.0);
        let cur = m.slot_at(env::block_timestamp());
        SlotInfo {
            market_id,
            num_slots: m.num_slots(),
            current_slot: cur,
            current_filled: cur.is_some_and(|s| self.observations.contains_key(&(m.id, s))),
            current_pending: cur.is_some_and(|s| self.slot_pending.contains_key(&(m.id, s))),
            filled_slots: m.filled_slots,
        }
    }

    pub fn observation_budget(&self) -> U128 {
        U128(self.budget())
    }

    pub fn is_pending(&self, pending_id: U64) -> bool {
        self.pending.contains_key(&pending_id.0)
    }

    // ---------- internal ----------

    fn view_of(&self, m: &Market) -> MarketView {
        let observations = (0..m.num_slots())
            .filter_map(|s| {
                self.observations.get(&(m.id, s)).map(|o| ObservationView {
                    slot: s,
                    price_micros: U64(o.price_micros),
                    sources_ok: o.sources_ok,
                    at: U64(o.at),
                    outlayer_request: o.outlayer_request.clone(),
                    pending_id: U64(o.pending_id),
                })
            })
            .collect();
        MarketView {
            id: U64(m.id),
            template: m.template,
            index: m.index,
            floor_micros: U64(m.floor_micros),
            cap_micros: U64(m.cap_micros),
            auction_commit_end: U64(m.auction_commit_end),
            auction_reveal_end: U64(m.auction_reveal_end),
            period_start: U64(m.period_start),
            period_end: U64(m.period_end),
            slot_ns: U64(m.slot_ns),
            num_slots: m.num_slots(),
            min_filled_slots: m.min_filled_slots,
            void_after: U64(m.void_after),
            status: m.status.clone(),
            filled_slots: m.filled_slots,
            settled_average_micros: m.settled_average_micros.map(U64),
            observations,
            oracle_version_key: m.oracle_version_key.clone(),
            oracle_input: m.oracle_input.clone(),
            opening_price_cents: m.opening_price_cents,
            pairs: U128(m.pairs),
            collateral: U128(m.collateral),
            bid_count: m.bid_count,
            created_by: m.created_by.clone(),
        }
    }

    fn market(&self, id: u64) -> Market {
        self.markets.get(&id).cloned().unwrap_or_else(|| env::panic_str("ERR_NO_MARKET"))
    }

    fn save(&mut self, m: Market) {
        self.markets.insert(m.id, m);
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
    fn pay(&self, account_id: AccountId, market_id: u64, amount: u128, restore_long: u128, restore_short: u128, restore_pairs: u128) {
        Promise::new(self.fusd_id.clone())
            .function_call(
                "ft_transfer".to_string(),
                json!({"receiver_id": account_id, "amount": U128(amount), "memo": "flatrate"}).to_string().into_bytes(),
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

// ---------- NEP-171 / NEP-177 / NEP-181 ----------

#[near]
impl NonFungibleTokenCore for Contract {
    #[payable]
    fn nft_transfer(&mut self, receiver_id: AccountId, token_id: TokenId, approval_id: Option<u64>, memo: Option<String>) {
        self.tokens.nft_transfer(receiver_id, token_id, approval_id, memo)
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        self.tokens.nft_transfer_call(receiver_id, token_id, approval_id, memo, msg)
    }

    fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        self.tokens.nft_token(token_id)
    }
}

#[near]
impl NonFungibleTokenResolver for Contract {
    #[private]
    fn nft_resolve_transfer(
        &mut self,
        previous_owner_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        approved_account_ids: Option<std::collections::HashMap<AccountId, u64>>,
    ) -> bool {
        self.tokens.nft_resolve_transfer(previous_owner_id, receiver_id, token_id, approved_account_ids)
    }
}

#[near]
impl NonFungibleTokenEnumeration for Contract {
    fn nft_total_supply(&self) -> U128 {
        self.tokens.nft_total_supply()
    }

    fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.tokens.nft_tokens(from_index, limit)
    }

    fn nft_supply_for_owner(&self, account_id: AccountId) -> U128 {
        self.tokens.nft_supply_for_owner(account_id)
    }

    fn nft_tokens_for_owner(&self, account_id: AccountId, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.tokens.nft_tokens_for_owner(account_id, from_index, limit)
    }
}

#[near]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        NFTContractMetadata {
            spec: NFT_METADATA_SPEC.to_string(),
            name: "Flatrate Cover Note".to_string(),
            symbol: "FLATNOTE".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        }
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
    fn note_title_helpers() {
        // 2026-11-21 00:00:00 UTC
        assert_eq!(utc_date(1_795_219_200_000_000_000), "2026-11-21");
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400_000_000_000), "2000-02-29");
        assert_eq!(claims_str(100_000_000), "100");
        assert_eq!(claims_str(1_500_000), "1.5");
        assert_eq!(claims_str(1), "0.000001");
        assert_eq!(usd_str(2_400_000), "$2.40");
        assert_eq!(usd_str(5_605_000), "$5.60");
    }

    #[test]
    fn band_rule() {
        // 3.98 x 0.6 = 2.388 -> 2.30 and 3.98 x 1.4 = 5.572 -> 5.50 (both rounded down to 10 cents)
        assert_eq!(band_from_average(3_980_000), Some((2_300_000, 5_500_000)));
        assert_eq!(band_from_average(4_000_000), Some((2_400_000, 5_600_000)));
        // too small to make a band
        assert_eq!(band_from_average(100_000), None);
        assert_eq!(band_from_average(0), None);
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
