//! near-workspaces sandbox tests for Flatrate Phase 1.
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
