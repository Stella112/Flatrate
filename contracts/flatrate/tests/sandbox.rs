//! near-workspaces sandbox tests for Flatrate (Phases 1, 2 and 4).
//! Needs `make build` first (reads target/wasm/*.wasm). Uses `mock-outlayer` in place of
//! outlayer.testnet; the mock is never deployed to testnet.

use near_workspaces::network::Sandbox;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{Gas, NearToken};
use near_workspaces::{Account, Contract, Worker};
use serde_json::{json, Value};

const ONE: u128 = 1_000_000; // 1 fUSD / 1 claim
const SEC: u64 = 1_000_000_000;
const FLOOR: u64 = 2_400_000;
const CAP: u64 = 5_600_000;
const SOURCES: [&str; 5] = ["akash", "vast", "runpod", "shadeform", "verda"];

struct Env {
    worker: Worker<Sandbox>,
    owner: Account,
    alice: Account,
    bob: Account,
    fusd: Contract,
    flat: Contract,
    mock: Contract,
}

fn wasm(name: &str) -> Vec<u8> {
    let p = format!("{}/../../target/wasm/{}.wasm", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {p}: {e}. Run `make build` first."))
}

async fn setup() -> anyhow::Result<Env> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let mk = |name: &'static str, near: u128| {
        let root = root.clone();
        async move {
            root.create_subaccount(name)
                .initial_balance(NearToken::from_near(near))
                .transact()
                .await?
                .into_result()
                .map_err(anyhow::Error::from)
        }
    };
    let owner = mk("owner", 50).await?;
    let alice = mk("alice", 20).await?;
    let bob = mk("bob", 20).await?;
    let fusd = mk("fusd", 20).await?.deploy(&wasm("fusd")).await?.into_result()?;
    let mock = mk("mock", 20).await?.deploy(&wasm("mock_outlayer")).await?.into_result()?;
    let flat = mk("flatrate", 30).await?.deploy(&wasm("flatrate")).await?.into_result()?;

    fusd.call("new").transact().await?.into_result()?;
    mock.call("new").transact().await?.into_result()?;
    flat.call("new")
        .args_json(json!({
            "owner_id": owner.id(),
            "fusd_id": fusd.id(),
            "outlayer_id": mock.id(),
            "oracle_project_id": format!("{}/flatrate-oracle", owner.id()),
            "oracle_version_key": "mock-version-sha256",
            "oracle_sources": SOURCES,
            "resource_limits": {"max_instructions": "10000000000", "max_memory_mb": 128, "max_execution_seconds": "60"},
        }))
        .transact()
        .await?
        .into_result()?;
    owner
        .call(fusd.id(), "storage_deposit")
        .args_json(json!({"account_id": flat.id()}))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?
        .into_result()?;
    for a in [&alice, &bob] {
        a.call(fusd.id(), "faucet").args_json(json!({})).transact().await?.into_result()?;
    }
    Ok(Env { worker, owner, alice, bob, fusd, flat, mock })
}

async fn now(w: &Worker<Sandbox>) -> anyhow::Result<u64> {
    Ok(w.view_block().await?.timestamp())
}

async fn advance_to(w: &Worker<Sandbox>, ts: u64) -> anyhow::Result<()> {
    for _ in 0..200 {
        if now(w).await? >= ts {
            return Ok(());
        }
        w.fast_forward(20).await?;
    }
    anyhow::bail!("could not reach timestamp {ts}")
}

struct Times {
    reveal_end: u64,
    period_start: u64,
    period_end: u64,
    void_after: u64,
}

/// Creates a market (auction 10s/10s, period 120s, void 120s after) and returns its times.
async fn create_market(e: &Env, min_obs: u32) -> anyhow::Result<(u64, Times)> {
    let t0 = now(&e.worker).await?;
    let t = Times {
        reveal_end: t0 + 20 * SEC,
        period_start: t0 + 20 * SEC,
        period_end: t0 + 140 * SEC,
        void_after: t0 + 260 * SEC,
    };
    let id: String = e
        .owner
        .call(e.flat.id(), "create_market")
        .args_json(json!({
            "index": "H100_HOURLY",
            "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string(),
            "auction_commit_end": (t0 + 10 * SEC).to_string(),
            "auction_reveal_end": t.reveal_end.to_string(),
            "period_start": t.period_start.to_string(),
            "period_end": t.period_end.to_string(),
            "min_observations": min_obs,
            "void_after": t.void_after.to_string(),
        }))
        .transact()
        .await?
        .into_result()?
        .json()?;
    Ok((id.parse()?, t))
}

/// Creates a market and opens it (Phase 1: no bids, so clear_auction just opens it).
async fn open_market(e: &Env, min_obs: u32) -> anyhow::Result<(u64, Times)> {
    let (id, t) = create_market(e, min_obs).await?;
    advance_to(&e.worker, t.reveal_end).await?;
    e.alice
        .call(e.flat.id(), "clear_auction")
        .args_json(json!({"market_id": id.to_string()}))
        .transact()
        .await?
        .into_result()?;
    Ok((id, t))
}

async fn mint(e: &Env, who: &Account, market: u64, amount: u128) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.fusd.id(), "ft_transfer_call")
        .args_json(json!({
            "receiver_id": e.flat.id(),
            "amount": amount.to_string(),
            "msg": json!({"action": "mint_pair", "market_id": market.to_string()}).to_string(),
        }))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

async fn fusd_balance(e: &Env, who: &Account) -> anyhow::Result<u128> {
    let v: String = e.fusd.view("ft_balance_of").args_json(json!({"account_id": who.id()})).await?.json()?;
    Ok(v.parse()?)
}

async fn claims(e: &Env, who: &Account, market: u64, side: &str) -> anyhow::Result<u128> {
    let v: String = e
        .flat
        .view("claim_balance_of")
        .args_json(json!({"account_id": who.id(), "market_id": market.to_string(), "side": side}))
        .await?
        .json()?;
    Ok(v.parse()?)
}

async fn market(e: &Env, id: u64) -> anyhow::Result<Value> {
    Ok(e.flat.view("get_market").args_json(json!({"market_id": id.to_string()})).await?.json()?)
}

/// Oracle output with these per-source prices; `price_micros` is their median unless given.
fn oracle_output(prices: &[Option<u64>], price: Option<u64>) -> String {
    let ok: Vec<u64> = prices.iter().flatten().copied().collect();
    let median = flatrate_shared::median(&ok);
    let sources: Vec<Value> = prices
        .iter()
        .enumerate()
        .map(|(i, p)| match p {
            Some(v) => json!({"name": SOURCES[i], "price_micros": v, "error": null}),
            None => json!({"name": SOURCES[i], "price_micros": null, "error": "http 503"}),
        })
        .collect();
    json!({
        "index": "H100_HOURLY",
        "price_micros": price.or(median),
        "sources": sources,
        "fetched_at": 1794441600u64,
        "request_id": "mock-request",
    })
    .to_string()
}

fn around(p: u64) -> Vec<Option<u64>> {
    vec![Some(p - 20_000), Some(p - 10_000), Some(p), Some(p + 10_000), Some(p + 20_000)]
}

async fn observe(e: &Env, market: u64, mode: &str, output: Option<String>) -> anyhow::Result<ExecutionFinalResult> {
    e.owner
        .call(e.mock.id(), "mock_set_next")
        .args_json(json!({"mode": mode, "output": output}))
        .transact()
        .await?
        .into_result()?;
    Ok(e.alice
        .call(e.flat.id(), "request_observation")
        .args_json(json!({"market_id": market.to_string()}))
        .deposit(NearToken::from_millinear(10))
        .gas(Gas::from_tgas(300))
        .transact()
        .await?)
}

fn fails_with(res: &ExecutionFinalResult, err: &str) -> bool {
    !res.is_success() && format!("{:?}", res.failures()).contains(err)
}

fn has_log(res: &ExecutionFinalResult, needle: &str) -> bool {
    res.logs().iter().any(|l| l.contains(needle))
}

async fn settle(e: &Env, who: &Account, id: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.flat.id(), "settle")
        .args_json(json!({"market_id": id.to_string()}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

async fn redeem(e: &Env, who: &Account, id: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.flat.id(), "redeem")
        .args_json(json!({"market_id": id.to_string()}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

// ---------------- Ledger ----------------

#[tokio::test]
async fn ledger_mint_transfer_redeem_pair() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    let start = fusd_balance(&e, &e.alice).await?;

    let r = mint(&e, &e.alice, id, 10 * ONE).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 10 * ONE);
    assert_eq!(claims(&e, &e.alice, id, "Short").await?, 10 * ONE);
    assert_eq!(fusd_balance(&e, &e.alice).await?, start - 10 * ONE);

    e.alice
        .call(e.flat.id(), "claim_transfer")
        .args_json(json!({"receiver_id": e.bob.id(), "market_id": id.to_string(), "side": "Long", "amount": (4 * ONE).to_string()}))
        .transact()
        .await?
        .into_result()?;
    assert_eq!(claims(&e, &e.bob, id, "Long").await?, 4 * ONE);
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 6 * ONE);

    let r = e
        .alice
        .call(e.flat.id(), "redeem_pair")
        .args_json(json!({"market_id": id.to_string(), "amount": (6 * ONE).to_string()}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 0);
    assert_eq!(claims(&e, &e.alice, id, "Short").await?, 4 * ONE);
    assert_eq!(fusd_balance(&e, &e.alice).await?, start - 4 * ONE);

    // Bob holds Long only: cannot redeem a pair.
    let r = e
        .bob
        .call(e.flat.id(), "redeem_pair")
        .args_json(json!({"market_id": id.to_string(), "amount": ONE.to_string()}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_INSUFFICIENT_CLAIMS"), "{:?}", r.failures());

    let m = market(&e, id).await?;
    assert_eq!(m["collateral"], (4 * ONE).to_string());
    assert_eq!(m["pairs"], (4 * ONE).to_string());
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, 4 * ONE);
    Ok(())
}

#[tokio::test]
async fn mint_refused_while_in_auction_and_refunded() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = create_market(&e, 3).await?;
    let start = fusd_balance(&e, &e.alice).await?;
    let r = mint(&e, &e.alice, id, 5 * ONE).await?;
    // The receiver panics with ERR_MARKET_NOT_OPEN; NEP-141 ft_resolve_transfer refunds.
    let _ = r;
    assert_eq!(fusd_balance(&e, &e.alice).await?, start, "fUSD refunded by NEP-141 resolve");
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 0);
    Ok(())
}

// ---------------- Full lifecycle ----------------

#[tokio::test]
async fn observe_settle_redeem_and_double_redeem() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = open_market(&e, 3).await?;
    mint(&e, &e.alice, id, 100 * ONE).await?.into_result()?;
    e.alice
        .call(e.flat.id(), "claim_transfer")
        .args_json(json!({"receiver_id": e.bob.id(), "market_id": id.to_string(), "side": "Short", "amount": (100 * ONE).to_string()}))
        .transact()
        .await?
        .into_result()?;

    for p in [3_980_000u64, 3_990_000, 4_000_000] {
        let r = observe(&e, id, "Respond", Some(oracle_output(&around(p), None))).await?;
        assert!(r.is_success(), "{:?}", r.failures());
        assert!(has_log(&r, "observation_recorded"), "{:?}", r.logs());
    }
    let m = market(&e, id).await?;
    assert_eq!(m["observations"].as_array().unwrap().len(), 3);
    assert_eq!(m["observations"][0]["outlayer_request"], "mock-request");
    assert_eq!(m["observations"][0]["sources_ok"], 5);

    // The OutLayer call carries the pinned version, resource limits, Json format and payer.
    let args: Value = serde_json::from_str(&e.mock.view("mock_last_args").await?.json::<String>()?)?;
    assert_eq!(args["source"]["Project"]["version_key"], "mock-version-sha256");
    assert_eq!(args["source"]["Project"]["project_id"], format!("{}/flatrate-oracle", e.owner.id()));
    assert_eq!(args["response_format"], "Json");
    assert_eq!(args["payer_account_id"], e.alice.id().to_string());
    assert_eq!(args["resource_limits"]["max_memory_mb"], 128);
    let input: Value = serde_json::from_str(args["input_data"].as_str().unwrap())?;
    assert_eq!(input["index"], "H100_HOURLY");

    advance_to(&e.worker, t.period_end).await?;
    let r = settle(&e, &e.bob, id).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    // mean(3.98, 3.99, 4.00) = 3.99 -> (3.99 - 2.40) / 3.20 = 0.496875
    assert_eq!(market(&e, id).await?["status"], json!({"Settled": {"long_payout_micros": 496_875}}));

    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    redeem(&e, &e.bob, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 49_687_500);
    assert_eq!(fusd_balance(&e, &e.bob).await? - b0, 50_312_500);
    assert_eq!(market(&e, id).await?["collateral"], "0");

    // Double redeem pays nothing.
    let r = redeem(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_NOTHING_TO_REDEEM"), "{:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 49_687_500);
    Ok(())
}

// ---------------- Settlement ----------------

#[tokio::test]
async fn settle_too_early_refused() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_PERIOD_NOT_ENDED"), "{:?}", r.failures());
    Ok(())
}

#[tokio::test]
async fn too_few_observations_waits_then_voids_then_second_settle_refused() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = open_market(&e, 3).await?;
    mint(&e, &e.alice, id, 10 * ONE).await?.into_result()?;
    observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?.into_result()?;

    advance_to(&e.worker, t.period_end).await?;
    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_TOO_FEW_OBSERVATIONS"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["status"], "Open");

    advance_to(&e.worker, t.void_after).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(market(&e, id).await?["status"], "Void");

    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_ALREADY_SETTLED"), "{:?}", r.failures());

    // Void pays 0.50 per claim on each side: 10 Long + 10 Short -> 10 fUSD.
    let a0 = fusd_balance(&e, &e.alice).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 10 * ONE);
    Ok(())
}

// ---------------- Oracle callback ----------------

#[tokio::test]
async fn oracle_non_outlayer_caller_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    // A made-up price with an id that was never issued.
    let r = e
        .alice
        .call(e.flat.id(), "on_observation")
        .args_json(json!({"market_id": id.to_string(), "pending_id": "999"}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_NOT_OUTLAYER"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["observations"].as_array().unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn oracle_unknown_request_id_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?.into_result()?;
    let used = market(&e, id).await?["observations"][0]["pending_id"].as_str().unwrap().to_string();
    // Replay the used id.
    let r = e
        .bob
        .call(e.flat.id(), "on_observation")
        .args_json(json!({"market_id": id.to_string(), "pending_id": used}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_UNKNOWN_REQUEST"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["observations"].as_array().unwrap().len(), 1);
    Ok(())
}

#[tokio::test]
async fn oracle_fewer_than_3_sources_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    let out = oracle_output(&[Some(3_900_000), None, Some(4_000_000), None, None], None);
    let r = observe(&e, id, "Respond", Some(out)).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "too_few_sources"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["observations"].as_array().unwrap().len(), 0);
    let pending: bool = e.flat.view("is_pending").args_json(json!({"pending_id": "0"})).await?.json()?;
    assert!(!pending, "request id is used up even when the result is rejected");
    Ok(())
}

#[tokio::test]
async fn oracle_out_of_range_price_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    // 10 x cap = 56.00; a median of 56.00 is out of range.
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(10 * CAP), None))).await?;
    assert!(has_log(&r, "price_out_of_range"), "{:?}", r.logs());
    // Zero price.
    let zeros = vec![Some(0), Some(0), Some(0), Some(0), Some(0)];
    let r = observe(&e, id, "Respond", Some(oracle_output(&zeros, None))).await?;
    assert!(has_log(&r, "price_out_of_range"), "{:?}", r.logs());
    // Claimed price that is not the median of the sources.
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), Some(4_500_000)))).await?;
    assert!(has_log(&r, "median_mismatch"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["observations"].as_array().unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn oracle_failed_run_and_refused_call_record_nothing() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, _) = open_market(&e, 3).await?;
    let r = observe(&e, id, "Null", None).await?;
    assert!(has_log(&r, "oracle_no_output"), "{:?}", r.logs());
    let r = observe(&e, id, "Panic", None).await?;
    assert!(has_log(&r, "outlayer_call_failed"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["observations"].as_array().unwrap().len(), 0);
    Ok(())
}

#[tokio::test]
async fn request_observation_outside_period_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_market(&e, 3).await?;
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(fails_with(&r, "ERR_MARKET_NOT_OPEN"), "{:?}", r.failures());
    advance_to(&e.worker, t.reveal_end).await?;
    e.alice.call(e.flat.id(), "clear_auction").args_json(json!({"market_id": id.to_string()})).transact().await?.into_result()?;
    advance_to(&e.worker, t.period_end).await?;
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(fails_with(&r, "ERR_OUTSIDE_PERIOD"), "{:?}", r.failures());
    let _ = t.period_start;
    Ok(())
}

#[tokio::test]
async fn create_market_is_owner_only() -> anyhow::Result<()> {
    let e = setup().await?;
    let t0 = now(&e.worker).await?;
    let r = e
        .alice
        .call(e.flat.id(), "create_market")
        .args_json(json!({
            "index": "H100_HOURLY", "floor_micros": "1", "cap_micros": "2",
            "auction_commit_end": (t0 + 10 * SEC).to_string(), "auction_reveal_end": (t0 + 20 * SEC).to_string(),
            "period_start": (t0 + 20 * SEC).to_string(), "period_end": (t0 + 30 * SEC).to_string(),
            "min_observations": 1, "void_after": (t0 + 40 * SEC).to_string(),
        }))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());
    Ok(())
}

// ---------------- Sealed opening auction ----------------

struct AuctionTimes {
    commit_end: u64,
    reveal_end: u64,
}

/// Creates a market with the given commit and reveal windows (seconds from now).
async fn create_auction_market(e: &Env, commit_s: u64, reveal_s: u64) -> anyhow::Result<(u64, AuctionTimes)> {
    let t0 = now(&e.worker).await?;
    let t = AuctionTimes { commit_end: t0 + commit_s * SEC, reveal_end: t0 + (commit_s + reveal_s) * SEC };
    let id: String = e
        .owner
        .call(e.flat.id(), "create_market")
        .args_json(json!({
            "index": "H100_HOURLY",
            "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string(),
            "auction_commit_end": t.commit_end.to_string(),
            "auction_reveal_end": t.reveal_end.to_string(),
            "period_start": t.reveal_end.to_string(),
            "period_end": (t.reveal_end + 120 * SEC).to_string(),
            "min_observations": 3,
            "void_after": (t.reveal_end + 240 * SEC).to_string(),
        }))
        .transact()
        .await?
        .into_result()?
        .json()?;
    Ok((id.parse()?, t))
}

#[derive(Clone)]
struct TestBid {
    side: &'static str,
    qty: u64,
    limit: u8,
    salt: String,
    escrow: u128,
}

impl TestBid {
    fn new(side: &'static str, qty: u64, limit: u8, salt: &str, extra: u128) -> Self {
        Self { side, qty, limit, salt: salt.to_string(), escrow: qty as u128 * limit as u128 * 10_000 + extra }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// sha256(borsh(market_id: u64, side: u8 enum index, qty: u64, limit_cents: u8, salt: String)).
fn bid_hash(market: u64, b: &TestBid) -> String {
    use sha2::{Digest, Sha256};
    let side: u8 = if b.side == "Long" { 0 } else { 1 };
    let bytes = borsh::to_vec(&(market, side, b.qty, b.limit, b.salt.clone())).unwrap();
    hex(Sha256::digest(&bytes).as_slice())
}

fn commit_msg(market: u64, b: &TestBid) -> String {
    json!({"action": "commit_bid", "market_id": market.to_string(), "hash": bid_hash(market, b)}).to_string()
}

async fn commit(e: &Env, who: &Account, market: u64, b: &TestBid) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.fusd.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": e.flat.id(), "amount": b.escrow.to_string(), "msg": commit_msg(market, b)}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

fn reveal_args(market: u64, b: &TestBid) -> Value {
    json!({"market_id": market.to_string(), "side": b.side, "qty": b.qty.to_string(), "limit_cents": b.limit, "salt": b.salt})
}

async fn reveal(e: &Env, who: &Account, market: u64, b: &TestBid) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "reveal_bid").args_json(reveal_args(market, b)).gas(Gas::from_tgas(30)).transact().await?)
}

async fn clear(e: &Env, market: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(e.alice
        .call(e.flat.id(), "clear_auction")
        .args_json(json!({"market_id": market.to_string()}))
        .gas(Gas::from_tgas(300))
        .transact()
        .await?)
}

async fn refund_balance(e: &Env, who: &Account) -> anyhow::Result<u128> {
    let v: String = e.flat.view("refund_balance_of").args_json(json!({"account_id": who.id()})).await?.json()?;
    Ok(v.parse()?)
}

async fn withdraw(e: &Env, who: &Account) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "withdraw_refund").gas(Gas::from_tgas(50)).transact().await?)
}

async fn bids(e: &Env, market: u64) -> anyhow::Result<Vec<Value>> {
    Ok(e.flat
        .view("get_bids")
        .args_json(json!({"market_id": market.to_string(), "limit": 200}))
        .await?
        .json()?)
}

/// A receipt in the transaction failed with `err` (the outer ft_transfer_call may still succeed
/// because NEP-141 `ft_resolve_transfer` refunds the sender).
fn receipt_failed_with(res: &ExecutionFinalResult, err: &str) -> bool {
    format!("{:?}", res.failures()).contains(err)
}

#[tokio::test]
async fn auction_two_bid_match() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    // Alice deposits 1 fUSD more than she needs, so the deposit does not reveal the bid.
    let long = TestBid::new("Long", 10, 60, "alice-salt-1", ONE);
    let short = TestBid::new("Short", 10, 45, "bob-salt-1", 0);

    // The contract's view computes the same commitment as the client.
    let view_hash: String = e.flat.view("bid_hash").args_json(reveal_args(id, &long)).await?.json()?;
    assert_eq!(view_hash, bid_hash(id, &long));

    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    assert_eq!(market(&e, id).await?["bid_count"], 2);
    assert_eq!(fusd_balance(&e, &e.alice).await?, a0 - long.escrow);
    // Before reveal, the stored bid shows only the hash and escrow.
    assert!(bids(&e, id).await?[0]["revealed"].is_null());

    advance_to(&e.worker, t.commit_end).await?;
    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;

    advance_to(&e.worker, t.reveal_end).await?;
    let r = clear(&e, id).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    let m = market(&e, id).await?;
    assert_eq!(m["status"], "Open");
    assert_eq!(m["opening_price_cents"], 55);
    assert_eq!(m["pairs"], (10 * ONE).to_string());
    assert_eq!(m["collateral"], (10 * ONE).to_string());
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 10 * ONE);
    assert_eq!(claims(&e, &e.bob, id, "Short").await?, 10 * ONE);
    // Alice paid 10 x 55c = 5.50 of 7.00; Bob paid 10 x 45c = 4.50, all of his escrow.
    assert_eq!(refund_balance(&e, &e.alice).await?, 1_500_000);
    assert_eq!(refund_balance(&e, &e.bob).await?, 0);

    withdraw(&e, &e.alice).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, a0 - 5_500_000);
    assert_eq!(fusd_balance(&e, &e.bob).await?, b0 - 4_500_000);
    let r = withdraw(&e, &e.bob).await?;
    assert!(fails_with(&r, "ERR_NOTHING_TO_REFUND"), "{:?}", r.failures());
    let r = withdraw(&e, &e.alice).await?;
    assert!(fails_with(&r, "ERR_NOTHING_TO_REFUND"), "a refund is paid once: {:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, 10 * ONE);

    // Clearing twice is refused; minting works once the market is open.
    let r = clear(&e, id).await?;
    assert!(fails_with(&r, "ERR_NOT_IN_AUCTION"), "{:?}", r.failures());
    mint(&e, &e.alice, id, ONE).await?.into_result()?;
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 11 * ONE);
    Ok(())
}

#[tokio::test]
async fn auction_tie_break() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    // Long up to 70c and Short up to 70c match 5 at every P in 30..=70; the rule picks 50.
    let long = TestBid::new("Long", 5, 70, "l", 0);
    let short = TestBid::new("Short", 5, 70, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, t.commit_end).await?;
    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, t.reveal_end).await?;
    clear(&e, id).await?.into_result()?;
    assert_eq!(market(&e, id).await?["opening_price_cents"], 50);
    // Each side paid 2.50 of 3.50 and gets 1.00 back.
    assert_eq!(refund_balance(&e, &e.alice).await?, ONE);
    assert_eq!(refund_balance(&e, &e.bob).await?, ONE);
    Ok(())
}

#[tokio::test]
async fn auction_partial_fill() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    // Short 7 at 50c (P >= 50). Longs in commit order: 5@60, 5@55, 5@60.
    // At P = 50 Long has 15 and Short 7. Fill by limit, then commit order: bid 0 gets 5,
    // bid 2 gets 2 (the marginal bid, partly filled), bid 1 gets nothing.
    let l0 = TestBid::new("Long", 5, 60, "l0", 0);
    let l1 = TestBid::new("Long", 5, 55, "l1", 0);
    let l2 = TestBid::new("Long", 5, 60, "l2", 0);
    let s = TestBid::new("Short", 7, 50, "s", 0);
    commit(&e, &e.alice, id, &l0).await?.into_result()?;
    commit(&e, &e.bob, id, &l1).await?.into_result()?;
    commit(&e, &e.alice, id, &l2).await?.into_result()?;
    commit(&e, &e.bob, id, &s).await?.into_result()?;
    advance_to(&e.worker, t.commit_end).await?;
    reveal(&e, &e.alice, id, &l0).await?.into_result()?;
    reveal(&e, &e.bob, id, &l1).await?.into_result()?;
    reveal(&e, &e.alice, id, &l2).await?.into_result()?;
    reveal(&e, &e.bob, id, &s).await?.into_result()?;
    advance_to(&e.worker, t.reveal_end).await?;
    clear(&e, id).await?.into_result()?;

    let m = market(&e, id).await?;
    assert_eq!(m["opening_price_cents"], 50);
    assert_eq!(m["pairs"], (7 * ONE).to_string());
    let b = bids(&e, id).await?;
    assert_eq!(b[0]["filled_qty"], "5");
    assert_eq!(b[1]["filled_qty"], "0");
    assert_eq!(b[2]["filled_qty"], "2");
    assert_eq!(b[3]["filled_qty"], "7");
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 7 * ONE);
    assert_eq!(claims(&e, &e.bob, id, "Long").await?, 0);
    assert_eq!(claims(&e, &e.bob, id, "Short").await?, 7 * ONE);
    // Alice: escrow 3.00 + 3.00, paid 7 x 0.50 = 3.50 -> 2.50 back.
    // Bob: escrow 2.75 (unfilled) + 3.50, paid 7 x 0.50 = 3.50 -> 2.75 back.
    assert_eq!(refund_balance(&e, &e.alice).await?, 2_500_000);
    assert_eq!(refund_balance(&e, &e.bob).await?, 2_750_000);
    Ok(())
}

#[tokio::test]
async fn auction_unrevealed_bid_refunded() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    let long = TestBid::new("Long", 10, 60, "never-revealed", 250_000);
    let short = TestBid::new("Short", 10, 45, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, t.commit_end).await?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, t.reveal_end).await?;
    clear(&e, id).await?.into_result()?;

    let m = market(&e, id).await?;
    assert_eq!(m["status"], "Open");
    assert!(m["opening_price_cents"].is_null(), "nothing matched");
    assert_eq!(m["pairs"], "0");
    assert_eq!(refund_balance(&e, &e.alice).await?, long.escrow);
    assert_eq!(refund_balance(&e, &e.bob).await?, short.escrow);
    withdraw(&e, &e.alice).await?.into_result()?;
    withdraw(&e, &e.bob).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, a0);
    assert_eq!(fusd_balance(&e, &e.bob).await?, b0);
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, 0);
    Ok(())
}

#[tokio::test]
async fn auction_wrong_salt_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    let long = TestBid::new("Long", 10, 60, "right-salt", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    // Escrow covers exactly 10 x 60c; a bid that needs more is refused at reveal.
    let under = TestBid { escrow: 1_000_000, ..TestBid::new("Long", 10, 60, "under", 0) };
    commit(&e, &e.alice, id, &under).await?.into_result()?;
    advance_to(&e.worker, t.commit_end).await?;

    let wrong = TestBid { salt: "wrong-salt".to_string(), ..long.clone() };
    let r = reveal(&e, &e.alice, id, &wrong).await?;
    assert!(fails_with(&r, "ERR_BAD_REVEAL"), "{:?}", r.failures());
    // Someone else cannot reveal Alice's bid, even with the right fields.
    let r = reveal(&e, &e.bob, id, &long).await?;
    assert!(fails_with(&r, "ERR_BAD_REVEAL"), "{:?}", r.failures());
    let r = reveal(&e, &e.alice, id, &under).await?;
    assert!(fails_with(&r, "ERR_ESCROW_TOO_LOW"), "{:?}", r.failures());

    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_ALREADY_REVEALED"), "{:?}", r.failures());
    let b = bids(&e, id).await?;
    assert_eq!(b[0]["revealed"]["limit_cents"], 60);
    assert!(b[1]["revealed"].is_null());
    Ok(())
}

#[tokio::test]
async fn auction_late_reveal_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 60, 60).await?;
    let long = TestBid::new("Long", 10, 60, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    // Reveal before the commit window closes is refused.
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_REVEAL_NOT_OPEN"), "{:?}", r.failures());
    // Clearing before the reveal window ends is refused.
    let r = clear(&e, id).await?;
    assert!(fails_with(&r, "ERR_REVEAL_NOT_ENDED"), "{:?}", r.failures());

    advance_to(&e.worker, t.commit_end).await?;
    // A commit after the window closes is refused and NEP-141 returns the fUSD.
    let b0 = fusd_balance(&e, &e.bob).await?;
    let late = TestBid::new("Short", 10, 45, "late", 0);
    let r = commit(&e, &e.bob, id, &late).await?;
    assert!(receipt_failed_with(&r, "ERR_COMMIT_ENDED"), "{:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.bob).await?, b0);
    assert_eq!(market(&e, id).await?["bid_count"], 1);

    advance_to(&e.worker, t.reveal_end).await?;
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_REVEAL_ENDED"), "{:?}", r.failures());
    clear(&e, id).await?.into_result()?;
    assert_eq!(refund_balance(&e, &e.alice).await?, long.escrow);
    Ok(())
}

#[tokio::test]
async fn auction_200_bids_clear_within_gas() -> anyhow::Result<()> {
    use near_workspaces::operations::Function;
    let e = setup().await?;
    let (id, t) = create_auction_market(&e, 900, 600).await?;
    // 100 Long bids from Alice and 100 Short bids from Bob, spread over all limits, so most
    // bids leave a refund and the marginal fill is partial. The contract does not merge bids
    // by account, so this costs the same gas as 200 different bidders.
    let make = |side: &'static str, i: u64| {
        let limit = 1 + ((i * 37) % 99) as u8;
        TestBid::new(side, 1 + i % 7, limit, &format!("{side}-{i}"), i as u128 * 1_000)
    };
    let longs: Vec<TestBid> = (0..100).map(|i| make("Long", i)).collect();
    let shorts: Vec<TestBid> = (0..100).map(|i| make("Short", i)).collect();

    for (who, list) in [(&e.alice, &longs), (&e.bob, &shorts)] {
        for chunk in list.chunks(6) {
            let mut tx = who.batch(e.fusd.id());
            for b in chunk {
                tx = tx.call(
                    Function::new("ft_transfer_call")
                        .args_json(json!({"receiver_id": e.flat.id(), "amount": b.escrow.to_string(), "msg": commit_msg(id, b)}))
                        .deposit(NearToken::from_yoctonear(1))
                        .gas(Gas::from_tgas(45)),
                );
            }
            let r = tx.transact().await?;
            assert!(r.failures().is_empty(), "{:?}", r.failures());
        }
    }
    assert_eq!(market(&e, id).await?["bid_count"], 200);
    // Bid 201 is refused and refunded.
    let extra = TestBid::new("Long", 1, 50, "extra", 0);
    let r = commit(&e, &e.alice, id, &extra).await?;
    assert!(receipt_failed_with(&r, "ERR_TOO_MANY_BIDS"), "{:?}", r.failures());

    advance_to(&e.worker, t.commit_end).await?;
    for (who, list) in [(&e.alice, &longs), (&e.bob, &shorts)] {
        for chunk in list.chunks(20) {
            let mut tx = who.batch(e.flat.id());
            for b in chunk {
                tx = tx.call(Function::new("reveal_bid").args_json(reveal_args(id, b)).gas(Gas::from_tgas(14)));
            }
            let r = tx.transact().await?;
            assert!(r.failures().is_empty(), "{:?}", r.failures());
        }
    }

    advance_to(&e.worker, t.reveal_end).await?;
    let r = clear(&e, id).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    // `outcome()` is only the transaction-to-receipt step; the function runs in the receipt
    // executed by the flatrate account.
    let clear_gas: u64 = r
        .receipt_outcomes()
        .iter()
        .filter(|o| o.executor_id == *e.flat.id())
        .map(|o| o.gas_burnt.as_gas())
        .sum();
    println!(
        "CLEAR_200_GAS clear_auction receipt gas_burnt = {} ({:.2} TGas); transaction total = {} ({:.2} TGas)",
        clear_gas,
        clear_gas as f64 / 1e12,
        r.total_gas_burnt.as_gas(),
        r.total_gas_burnt.as_gas() as f64 / 1e12
    );
    assert!(clear_gas > 1_000_000_000_000, "measured the function receipt, not the conversion step");
    assert!(clear_gas < 300_000_000_000_000);

    // Each pair is backed by exactly $1 and fUSD is conserved.
    let m = market(&e, id).await?;
    let pairs: u128 = m["pairs"].as_str().unwrap().parse()?;
    assert!(pairs > 0, "{m}");
    assert_eq!(m["collateral"], m["pairs"]);
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, pairs);
    assert_eq!(claims(&e, &e.bob, id, "Short").await?, pairs);
    let ra = refund_balance(&e, &e.alice).await?;
    let rb = refund_balance(&e, &e.bob).await?;
    let escrow: u128 = longs.iter().chain(&shorts).map(|b| b.escrow).sum();
    assert_eq!(pairs + ra + rb, escrow);
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, escrow);
    println!(
        "CLEAR_200_RESULT price_cents={} pairs={} refunds alice={} bob={}",
        m["opening_price_cents"], pairs, ra, rb
    );
    Ok(())
}

// ---------------- Cover Note NFT ----------------

async fn wrap(e: &Env, who: &Account, market: u64, side: &str, amount: u128) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.flat.id(), "wrap_note")
        .args_json(json!({"market_id": market.to_string(), "side": side, "amount": amount.to_string()}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await?)
}

async fn note_call(e: &Env, who: &Account, method: &str, token: &str) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.flat.id(), method)
        .args_json(json!({"token_id": token}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

async fn nft_token(e: &Env, token: &str) -> anyhow::Result<Value> {
    Ok(e.flat.view("nft_token").args_json(json!({"token_id": token})).await?.json()?)
}

#[tokio::test]
async fn note_wrap_transfer_new_owner_redeems_old_owner_cannot() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = open_market(&e, 3).await?;
    mint(&e, &e.alice, id, 100 * ONE).await?.into_result()?;

    let r = wrap(&e, &e.alice, id, "Long", 40 * ONE).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "nft_mint"), "{:?}", r.logs());
    let tok = nft_token(&e, "0").await?;
    assert_eq!(tok["owner_id"], e.alice.id().to_string());
    let title = tok["metadata"]["title"].as_str().unwrap().to_string();
    assert!(title.starts_with("Cover Note #0: 40 Long, H100, period ending "), "{title}");
    let extra: Value = serde_json::from_str(tok["metadata"]["extra"].as_str().unwrap())?;
    assert_eq!(extra["amount"], (40 * ONE).to_string());
    assert_eq!(extra["floor_micros"], FLOOR.to_string());
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 60 * ONE, "claims moved into the note");
    let meta: Value = e.flat.view("nft_metadata").await?.json()?;
    assert_eq!(meta["spec"], "nft-1.0.0");

    // Transfer the note like any NFT.
    e.alice
        .call(e.flat.id(), "nft_transfer")
        .args_json(json!({"receiver_id": e.bob.id(), "token_id": "0"}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;
    assert_eq!(nft_token(&e, "0").await?["owner_id"], e.bob.id().to_string());
    let bob_notes: Value = e.flat.view("nft_tokens_for_owner").args_json(json!({"account_id": e.bob.id()})).await?.json()?;
    assert_eq!(bob_notes.as_array().unwrap().len(), 1);

    // Before settlement the note cannot be redeemed.
    let r = note_call(&e, &e.bob, "redeem_note", "0").await?;
    assert!(fails_with(&r, "ERR_NOT_SETTLED"), "{:?}", r.failures());

    for p in [3_980_000u64, 3_990_000, 4_000_000] {
        observe(&e, id, "Respond", Some(oracle_output(&around(p), None))).await?.into_result()?;
    }
    advance_to(&e.worker, t.period_end).await?;
    settle(&e, &e.alice, id).await?.into_result()?;

    // The old owner cannot redeem it.
    let r = note_call(&e, &e.alice, "redeem_note", "0").await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());

    // The new owner can: 40 Long x 0.496875 = 19.875 fUSD.
    let b0 = fusd_balance(&e, &e.bob).await?;
    let r = note_call(&e, &e.bob, "redeem_note", "0").await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "nft_burn"), "{:?}", r.logs());
    assert_eq!(fusd_balance(&e, &e.bob).await? - b0, 19_875_000);
    assert!(nft_token(&e, "0").await?.is_null(), "burned");
    let supply: String = e.flat.view("nft_total_supply").await?.json()?;
    assert_eq!(supply, "0");
    let r = note_call(&e, &e.bob, "redeem_note", "0").await?;
    assert!(fails_with(&r, "ERR_NO_NOTE"), "paid once: {:?}", r.failures());

    // Alice redeems the rest: 60 Long + 100 Short; the market ends with no collateral left over
    // beyond rounding.
    let a0 = fusd_balance(&e, &e.alice).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 60 * 496_875 + 100 * 503_125);
    assert_eq!(market(&e, id).await?["collateral"], "0");
    Ok(())
}

#[tokio::test]
async fn note_unwrap_before_settle() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, t) = open_market(&e, 1).await?;
    mint(&e, &e.alice, id, 10 * ONE).await?.into_result()?;
    wrap(&e, &e.alice, id, "Short", 4 * ONE).await?.into_result()?;
    wrap(&e, &e.alice, id, "Short", 3_500_000).await?.into_result()?;
    assert_eq!(claims(&e, &e.alice, id, "Short").await?, 2_500_000);
    let title = nft_token(&e, "1").await?["metadata"]["title"].as_str().unwrap().to_string();
    assert!(title.starts_with("Cover Note #1: 3.5 Short"), "{title}");

    // Someone else cannot unwrap it; wrapping more than you hold fails.
    let r = note_call(&e, &e.bob, "unwrap_note", "0").await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());
    let r = wrap(&e, &e.alice, id, "Short", 3 * ONE).await?;
    assert!(fails_with(&r, "ERR_INSUFFICIENT_CLAIMS"), "{:?}", r.failures());

    let r = note_call(&e, &e.alice, "unwrap_note", "0").await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(claims(&e, &e.alice, id, "Short").await?, 6_500_000);
    assert!(nft_token(&e, "0").await?.is_null());
    let r = note_call(&e, &e.alice, "unwrap_note", "0").await?;
    assert!(fails_with(&r, "ERR_NO_NOTE"), "{:?}", r.failures());

    // After settlement a note can only be redeemed, not unwrapped.
    observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?.into_result()?;
    advance_to(&e.worker, t.period_end).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    let r = note_call(&e, &e.alice, "unwrap_note", "1").await?;
    assert!(fails_with(&r, "ERR_ALREADY_SETTLED"), "{:?}", r.failures());
    // 3.5 Short x 0.5 (avg 4.00 is the midpoint) = 1.75 fUSD.
    let a0 = fusd_balance(&e, &e.alice).await?;
    note_call(&e, &e.alice, "redeem_note", "1").await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 1_750_000);
    Ok(())
}

// ---------------- Upgrade from the deployed Phase 2 contract ----------------

#[tokio::test]
async fn migrate_from_phase2_state_keeps_markets_and_claims() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let owner = root.create_subaccount("owner").initial_balance(NearToken::from_near(50)).transact().await?.into_result()?;
    let alice = root.create_subaccount("alice").initial_balance(NearToken::from_near(20)).transact().await?.into_result()?;
    let fusd = root.create_subaccount("fusd").initial_balance(NearToken::from_near(20)).transact().await?.into_result()?
        .deploy(&wasm("fusd")).await?.into_result()?;
    let old = std::fs::read(format!("{}/tests/res/flatrate_phase2_deployed.wasm", env!("CARGO_MANIFEST_DIR")))?;
    let flat_acc = root.create_subaccount("flatrate").initial_balance(NearToken::from_near(30)).transact().await?.into_result()?;
    let flat = flat_acc.deploy(&old).await?.into_result()?;
    fusd.call("new").transact().await?.into_result()?;
    flat.call("new")
        .args_json(json!({
            "owner_id": owner.id(), "fusd_id": fusd.id(), "outlayer_id": "outlayer.test.near",
            "oracle_project_id": "x/flatrate-oracle", "oracle_version_key": "v1", "oracle_sources": SOURCES,
            "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
        }))
        .transact().await?.into_result()?;
    owner.call(fusd.id(), "storage_deposit").args_json(json!({"account_id": flat.id()}))
        .deposit(NearToken::from_millinear(10)).transact().await?.into_result()?;
    alice.call(fusd.id(), "faucet").args_json(json!({})).transact().await?.into_result()?;

    // Old state: one open market with claims, one market still in auction with a sealed bid.
    let t0 = worker.view_block().await?.timestamp();
    for (commit, reveal) in [(10u64, 20u64), (600, 1200)] {
        owner.call(flat.id(), "create_market")
            .args_json(json!({"index": "H100_HOURLY", "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string(),
                "auction_commit_end": (t0 + commit * SEC).to_string(), "auction_reveal_end": (t0 + reveal * SEC).to_string(),
                "period_start": (t0 + reveal * SEC).to_string(), "period_end": (t0 + (reveal + 600) * SEC).to_string(),
                "min_observations": 3, "void_after": (t0 + (reveal + 1200) * SEC).to_string()}))
            .transact().await?.into_result()?;
    }
    let msg = json!({"action": "commit_bid", "market_id": "1", "hash": "ab".repeat(32)}).to_string();
    alice.call(fusd.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": flat.id(), "amount": "2000000", "msg": msg}))
        .deposit(NearToken::from_yoctonear(1)).gas(Gas::from_tgas(100)).transact().await?.into_result()?;
    advance_to(&worker, t0 + 20 * SEC).await?;
    alice.call(flat.id(), "clear_auction").args_json(json!({"market_id": "0"})).transact().await?.into_result()?;
    let mint_msg = json!({"action": "mint_pair", "market_id": "0"}).to_string();
    alice.call(fusd.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": flat.id(), "amount": (7 * ONE).to_string(), "msg": mint_msg}))
        .deposit(NearToken::from_yoctonear(1)).gas(Gas::from_tgas(100)).transact().await?.into_result()?;
    let before0: Value = flat.view("get_market").args_json(json!({"market_id": "0"})).await?.json()?;
    let before1: Value = flat.view("get_market").args_json(json!({"market_id": "1"})).await?.json()?;
    let cfg_before: Value = flat.view("get_config").await?.json()?;

    // Upgrade: deploy the current build and call migrate in one transaction.
    let r = flat_acc
        .batch(flat.id())
        .deploy(&wasm("flatrate"))
        .call(near_workspaces::operations::Function::new("migrate").args_json(json!({})).gas(Gas::from_tgas(100)))
        .transact()
        .await?;
    assert!(r.is_success(), "{:?}", r.failures());
    // migrate is private: a second call from someone else fails.
    let r = alice.call(flat.id(), "migrate").args_json(json!({})).transact().await?;
    assert!(!r.is_success());

    let after0: Value = flat.view("get_market").args_json(json!({"market_id": "0"})).await?.json()?;
    let after1: Value = flat.view("get_market").args_json(json!({"market_id": "1"})).await?.json()?;
    assert_eq!(before0, after0);
    assert_eq!(before1, after1);
    assert_eq!(cfg_before, flat.view("get_config").await?.json::<Value>()?);
    let bids: Vec<Value> = flat.view("get_bids").args_json(json!({"market_id": "1"})).await?.json()?;
    assert_eq!(bids[0]["escrow"], "2000000");
    let long: String = flat.view("claim_balance_of")
        .args_json(json!({"account_id": alice.id(), "market_id": "0", "side": "Long"})).await?.json()?;
    assert_eq!(long, (7 * ONE).to_string());

    // New features work on migrated state.
    alice.call(flat.id(), "wrap_note")
        .args_json(json!({"market_id": "0", "side": "Long", "amount": (2 * ONE).to_string()}))
        .gas(Gas::from_tgas(50)).transact().await?.into_result()?;
    let tok: Value = flat.view("nft_token").args_json(json!({"token_id": "0"})).await?.json()?;
    assert_eq!(tok["owner_id"], alice.id().to_string());
    Ok(())
}
