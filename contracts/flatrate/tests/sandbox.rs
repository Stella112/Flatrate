//! near-workspaces sandbox tests for Flatrate (contract v2: templates, time slots, rolling).
//! Needs `make build` first (reads target/wasm/*.wasm). Uses `mock-outlayer` in place of
//! outlayer.testnet; the mock is never deployed to testnet.

use near_workspaces::network::Sandbox;
use near_workspaces::operations::Function;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{Gas, NearToken};
use near_workspaces::{Account, Contract, Worker};
use serde_json::{json, Value};

const ONE: u128 = 1_000_000; // 1 fUSD / 1 claim
const SEC: u64 = 1_000_000_000;
const FLOOR: u64 = 2_400_000;
const CAP: u64 = 5_600_000;
const SOURCES: [&str; 5] = ["akash", "vast", "runpod", "shadeform", "verda"];
const FIRST_MARKET: u64 = 10;

/// Template lengths in seconds: commit, reveal, period, slot, min filled slots, void after.
#[derive(Clone, Copy)]
struct Tpl(u64, u64, u64, u64, u32, u64);

impl Tpl {
    fn json(self) -> Value {
        json!({
            "commit_ns": (self.0 * SEC).to_string(), "reveal_ns": (self.1 * SEC).to_string(),
            "period_ns": (self.2 * SEC).to_string(), "slot_ns": (self.3 * SEC).to_string(),
            "min_filled_slots": self.4, "void_after_ns": (self.5 * SEC).to_string(),
        })
    }
}

/// Default test templates: DEMO is short (6 slots of 30 s, 3 needed); WEEKLY has 60 s auction
/// windows for the auction tests.
const DEMO: Tpl = Tpl(20, 20, 180, 30, 3, 120);
const WEEKLY: Tpl = Tpl(60, 60, 600, 60, 3, 300);

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

fn init_args(owner: &Account, fusd: &Contract, outlayer: &str, demo: Tpl, weekly: Tpl) -> Value {
    json!({
        "owner_id": owner.id(),
        "fusd_id": fusd.id(),
        "outlayer_id": outlayer,
        "oracle_project_id": format!("{}/flatrate-oracle", owner.id()),
        "oracle_version_key": "mock-version-sha256",
        "oracle_sources": SOURCES,
        "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
        "demo_template": demo.json(),
        "weekly_template": weekly.json(),
        "observation_deposit": "10000000000000000000000",
    })
}

async fn setup() -> anyhow::Result<Env> {
    setup_with(DEMO, WEEKLY).await
}

async fn setup_with(demo: Tpl, weekly: Tpl) -> anyhow::Result<Env> {
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
    flat.call("new").args_json(init_args(&owner, &fusd, mock.id().as_str(), demo, weekly)).transact().await?.into_result()?;
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
    for _ in 0..400 {
        if now(w).await? >= ts {
            return Ok(());
        }
        w.fast_forward(10).await?;
    }
    anyhow::bail!("could not reach timestamp {ts}")
}

fn n(v: &Value) -> u64 {
    v.as_str().map(|s| s.parse().unwrap()).unwrap_or_else(|| v.as_u64().unwrap())
}

async fn market(e: &Env, id: u64) -> anyhow::Result<Value> {
    Ok(e.flat.view("get_market").args_json(json!({"market_id": id.to_string()})).await?.json()?)
}

/// Owner creates the first market of `template` with the given band.
async fn create(e: &Env, template: &str, floor: u64, cap: u64) -> anyhow::Result<u64> {
    let id: String = e
        .owner
        .call(e.flat.id(), "create_market")
        .args_json(json!({"template": template, "floor_micros": floor.to_string(), "cap_micros": cap.to_string()}))
        .transact()
        .await?
        .into_result()?
        .json()?;
    Ok(id.parse()?)
}

async fn clear(e: &Env, id: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(e.alice
        .call(e.flat.id(), "clear_auction")
        .args_json(json!({"market_id": id.to_string()}))
        .gas(Gas::from_tgas(300))
        .transact()
        .await?)
}

/// Creates the DEMO market and opens it with no bids. Returns its id.
async fn open_demo(e: &Env) -> anyhow::Result<u64> {
    let id = create(e, "DEMO", FLOOR, CAP).await?;
    let m = market(e, id).await?;
    advance_to(&e.worker, n(&m["auction_reveal_end"])).await?;
    clear(e, id).await?.into_result()?;
    Ok(id)
}

/// Moves time to the start of slot `slot` of market `id` (plus a second).
async fn to_slot(e: &Env, id: u64, slot: u64) -> anyhow::Result<()> {
    let m = market(e, id).await?;
    advance_to(&e.worker, n(&m["period_start"]) + slot * n(&m["slot_ns"]) + SEC).await
}

async fn to_period_end(e: &Env, id: u64) -> anyhow::Result<()> {
    let m = market(e, id).await?;
    advance_to(&e.worker, n(&m["period_end"])).await
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
    json!({"index": "H100_HOURLY", "price_micros": price.or(median), "sources": sources, "fetched_at": 1794441600u64, "request_id": "mock-request"})
        .to_string()
}

fn around(p: u64) -> Vec<Option<u64>> {
    vec![Some(p - 20_000), Some(p - 10_000), Some(p), Some(p + 10_000), Some(p + 20_000)]
}

async fn set_mock(e: &Env, mode: &str, output: Option<String>) -> anyhow::Result<()> {
    e.owner.call(e.mock.id(), "mock_set_next").args_json(json!({"mode": mode, "output": output})).transact().await?.into_result()?;
    Ok(())
}

/// Requests an observation for the current slot (no deposit: the contract pays OutLayer).
async fn request(e: &Env, who: &Account, market: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.flat.id(), "request_observation")
        .args_json(json!({"market_id": market.to_string()}))
        .gas(Gas::from_tgas(300))
        .transact()
        .await?)
}

async fn observe(e: &Env, market: u64, mode: &str, output: Option<String>) -> anyhow::Result<ExecutionFinalResult> {
    set_mock(e, mode, output).await?;
    request(e, &e.alice, market).await
}

/// Moves to slot `slot` and records one observation there through the mock.
async fn observe_in_slot(e: &Env, market: u64, slot: u64, price: u64) -> anyhow::Result<()> {
    to_slot(e, market, slot).await?;
    let r = observe(e, market, "Respond", Some(oracle_output(&around(price), None))).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "observation_recorded"), "{:?}", r.logs());
    Ok(())
}

fn fails_with(res: &ExecutionFinalResult, err: &str) -> bool {
    !res.is_success() && format!("{:?}", res.failures()).contains(err)
}

fn has_log(res: &ExecutionFinalResult, needle: &str) -> bool {
    res.logs().iter().any(|l| l.contains(needle))
}

async fn settle(e: &Env, who: &Account, id: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "settle").args_json(json!({"market_id": id.to_string()})).gas(Gas::from_tgas(100)).transact().await?)
}

async fn redeem(e: &Env, who: &Account, id: u64) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "redeem").args_json(json!({"market_id": id.to_string()})).gas(Gas::from_tgas(100)).transact().await?)
}

async fn roll(e: &Env, who: &Account, template: &str) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "roll_market").args_json(json!({"template": template})).gas(Gas::from_tgas(50)).transact().await?)
}

async fn slot_info(e: &Env, id: u64) -> anyhow::Result<Value> {
    Ok(e.flat.view("slot_info").args_json(json!({"market_id": id.to_string()})).await?.json()?)
}

// ---------------- Markets and templates ----------------

#[tokio::test]
async fn create_market_is_owner_only_and_once_per_template() -> anyhow::Result<()> {
    let e = setup().await?;
    let r = e
        .alice
        .call(e.flat.id(), "create_market")
        .args_json(json!({"template": "DEMO", "floor_micros": "1", "cap_micros": "2"}))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());
    let id = create(&e, "DEMO", FLOOR, CAP).await?;
    assert_eq!(id, FIRST_MARKET, "market ids start at 10");
    let m = market(&e, id).await?;
    assert_eq!(n(&m["auction_reveal_end"]) - n(&m["auction_commit_end"]), DEMO.1 * SEC);
    assert_eq!(n(&m["period_end"]) - n(&m["period_start"]), DEMO.2 * SEC);
    assert_eq!(m["num_slots"], 6);
    assert_eq!(m["min_filled_slots"], 3);
    let r = e
        .owner
        .call(e.flat.id(), "create_market")
        .args_json(json!({"template": "DEMO", "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string()}))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_ALREADY_CREATED"), "{:?}", r.failures());
    assert_eq!(create(&e, "WEEKLY", FLOOR, CAP).await?, FIRST_MARKET + 1);
    let cfg: Value = e.flat.view("get_config").await?.json()?;
    assert_eq!(cfg["demo_template"]["min_filled_slots"], 3);
    assert!(cfg["observation_budget"].as_str().unwrap().parse::<u128>()? > 0);
    Ok(())
}

// ---------------- Ledger ----------------

#[tokio::test]
async fn ledger_mint_transfer_redeem_pair() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
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
    let r = e
        .alice
        .call(e.flat.id(), "redeem_pair")
        .args_json(json!({"market_id": id.to_string(), "amount": (6 * ONE).to_string()}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.alice).await?, start - 4 * ONE);
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
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, 4 * ONE);
    Ok(())
}

#[tokio::test]
async fn mint_refused_while_in_auction_and_refunded() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = create(&e, "DEMO", FLOOR, CAP).await?;
    let start = fusd_balance(&e, &e.alice).await?;
    let _ = mint(&e, &e.alice, id, 5 * ONE).await?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, start, "fUSD refunded by NEP-141 resolve");
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 0);
    Ok(())
}

#[tokio::test]
async fn observe_settle_redeem_and_double_redeem() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    mint(&e, &e.alice, id, 100 * ONE).await?.into_result()?;
    e.alice
        .call(e.flat.id(), "claim_transfer")
        .args_json(json!({"receiver_id": e.bob.id(), "market_id": id.to_string(), "side": "Short", "amount": (100 * ONE).to_string()}))
        .transact()
        .await?
        .into_result()?;
    for (slot, p) in [(0u64, 3_980_000u64), (1, 3_990_000), (2, 4_000_000)] {
        observe_in_slot(&e, id, slot, p).await?;
    }
    let m = market(&e, id).await?;
    assert_eq!(m["filled_slots"], 3);
    assert_eq!(m["observations"][0]["outlayer_request"], "mock-request");
    assert_eq!(m["observations"][2]["slot"], 2);

    // The OutLayer call carries the pinned version, limits, Json format, and the contract as payer.
    let args: Value = serde_json::from_str(&e.mock.view("mock_last_args").await?.json::<String>()?)?;
    assert_eq!(args["source"]["Project"]["version_key"], "mock-version-sha256");
    assert_eq!(args["response_format"], "Json");
    assert_eq!(args["payer_account_id"], e.flat.id().to_string(), "refunds go back to the observation budget");
    assert_eq!(args["resource_limits"]["max_memory_mb"], 128);

    to_period_end(&e, id).await?;
    settle(&e, &e.bob, id).await?.into_result()?;
    let m = market(&e, id).await?;
    // mean(3.98, 3.99, 4.00) = 3.99 -> (3.99 - 2.40) / 3.20 = 0.496875
    assert_eq!(m["status"], json!({"Settled": {"long_payout_micros": 496_875}}));
    assert_eq!(m["settled_average_micros"], "3990000");
    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    redeem(&e, &e.bob, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 49_687_500);
    assert_eq!(fusd_balance(&e, &e.bob).await? - b0, 50_312_500);
    assert_eq!(market(&e, id).await?["collateral"], "0");
    let r = redeem(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_NOTHING_TO_REDEEM"), "{:?}", r.failures());
    Ok(())
}

// ---------------- Oracle callback ----------------

#[tokio::test]
async fn oracle_non_outlayer_caller_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    let r = e
        .alice
        .call(e.flat.id(), "on_observation")
        .args_json(json!({"market_id": id.to_string(), "pending_id": "999"}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_NOT_OUTLAYER"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["filled_slots"], 0);
    Ok(())
}

#[tokio::test]
async fn oracle_unknown_request_id_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    observe_in_slot(&e, id, 0, 4_000_000).await?;
    let used = market(&e, id).await?["observations"][0]["pending_id"].as_str().unwrap().to_string();
    let r = e
        .bob
        .call(e.flat.id(), "on_observation")
        .args_json(json!({"market_id": id.to_string(), "pending_id": used}))
        .gas(Gas::from_tgas(50))
        .transact()
        .await?;
    assert!(fails_with(&r, "ERR_UNKNOWN_REQUEST"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["filled_slots"], 1);
    Ok(())
}

#[tokio::test]
async fn oracle_fewer_than_3_sources_rejected_and_slot_freed() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    to_slot(&e, id, 0).await?;
    let out = oracle_output(&[Some(3_900_000), None, Some(4_000_000), None, None], None);
    let r = observe(&e, id, "Respond", Some(out)).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "too_few_sources"), "{:?}", r.logs());
    let s = slot_info(&e, id).await?;
    assert_eq!(s["current_slot"], 0);
    assert_eq!(s["current_filled"], false);
    assert_eq!(s["current_pending"], false, "rejected result frees the slot");
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(has_log(&r, "observation_recorded"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["observations"][0]["slot"], 0);
    Ok(())
}

#[tokio::test]
async fn oracle_out_of_range_price_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    to_slot(&e, id, 0).await?;
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(10 * CAP), None))).await?;
    assert!(has_log(&r, "price_out_of_range"), "{:?}", r.logs());
    let zeros = vec![Some(0), Some(0), Some(0), Some(0), Some(0)];
    let r = observe(&e, id, "Respond", Some(oracle_output(&zeros, None))).await?;
    assert!(has_log(&r, "price_out_of_range"), "{:?}", r.logs());
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), Some(4_500_000)))).await?;
    assert!(has_log(&r, "median_mismatch"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["filled_slots"], 0);
    Ok(())
}

#[tokio::test]
async fn oracle_failed_run_and_refused_call_record_nothing() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    to_slot(&e, id, 0).await?;
    let r = observe(&e, id, "Null", None).await?;
    assert!(has_log(&r, "oracle_no_output"), "{:?}", r.logs());
    let r = observe(&e, id, "Panic", None).await?;
    assert!(has_log(&r, "outlayer_call_failed"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["filled_slots"], 0);
    Ok(())
}

// ---------------- Time slots ----------------

#[tokio::test]
async fn second_request_in_filled_slot_fails() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    observe_in_slot(&e, id, 0, 4_000_000).await?;
    let r = request(&e, &e.bob, id).await?;
    assert!(fails_with(&r, "ERR_SLOT_FILLED"), "{:?}", r.failures());
    observe_in_slot(&e, id, 1, 4_010_000).await?;
    assert_eq!(market(&e, id).await?["filled_slots"], 2);
    Ok(())
}

#[tokio::test]
async fn second_request_while_pending_fails_and_late_callback_lands_in_its_slot() -> anyhow::Result<()> {
    // 10 s slots: crossing one slot boundary stays well inside the 200-block yield window
    // (OutLayer's window is the same; a held request older than that times out to `null`).
    let e = setup_with(Tpl(20, 20, 60, 10, 3, 120), WEEKLY).await?;
    let id = open_demo(&e).await?;
    to_slot(&e, id, 0).await?;
    set_mock(&e, "Hold", None).await?;
    // The held transaction only finishes after the mock is resumed, so send it without waiting.
    let held = e
        .alice
        .call(e.flat.id(), "request_observation")
        .args_json(json!({"market_id": id.to_string()}))
        .gas(Gas::from_tgas(300))
        .transact_async()
        .await?;
    let mut pending = false;
    for _ in 0..50 {
        if slot_info(&e, id).await?["current_pending"] == true {
            pending = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    assert!(pending, "request never became pending");
    let r = request(&e, &e.bob, id).await?;
    assert!(fails_with(&r, "ERR_SLOT_FILLED"), "a slot with a request in flight is taken: {:?}", r.failures());

    // Move into slot 1 while the request is still held, then let OutLayer answer.
    to_slot(&e, id, 1).await?;
    assert_eq!(slot_info(&e, id).await?["current_slot"], 1);
    let resumed: bool = e
        .owner
        .call(e.mock.id(), "mock_resume")
        .args_json(json!({"output": oracle_output(&around(4_000_000), None)}))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?
        .into_result()?
        .json()?;
    assert!(resumed, "the held request was still inside the yield window");
    let done = held.await?;
    assert!(done.logs().iter().any(|l| l.contains("observation_recorded")), "{:?}", done.logs());
    let m = market(&e, id).await?;
    assert_eq!(m["filled_slots"], 1);
    assert_eq!(m["observations"][0]["slot"], 0, "late result is written into the slot it was requested for");
    Ok(())
}

#[tokio::test]
async fn failed_request_frees_the_slot() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    to_slot(&e, id, 0).await?;
    let r = observe(&e, id, "Null", None).await?;
    assert!(has_log(&r, "oracle_no_output"), "{:?}", r.logs());
    assert_eq!(slot_info(&e, id).await?["current_pending"], false);
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(has_log(&r, "observation_recorded"), "{:?}", r.logs());
    Ok(())
}

#[tokio::test]
async fn request_outside_period_fails() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = create(&e, "DEMO", FLOOR, CAP).await?;
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(fails_with(&r, "ERR_MARKET_NOT_OPEN"), "{:?}", r.failures());
    let m = market(&e, id).await?;
    advance_to(&e.worker, n(&m["auction_reveal_end"])).await?;
    clear(&e, id).await?.into_result()?;
    to_period_end(&e, id).await?;
    let r = observe(&e, id, "Respond", Some(oracle_output(&around(4_000_000), None))).await?;
    assert!(fails_with(&r, "ERR_NOT_IN_PERIOD"), "{:?}", r.failures());
    Ok(())
}

// ---------------- Settlement ----------------

#[tokio::test]
async fn settle_too_early_refused() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_PERIOD_NOT_ENDED"), "{:?}", r.failures());
    Ok(())
}

#[tokio::test]
async fn average_uses_filled_slots_only() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    // Slots 0, 2 and 5 filled; 1, 3, 4 empty.
    for (slot, p) in [(0u64, 3_000_000u64), (2, 4_000_000), (5, 5_000_000)] {
        observe_in_slot(&e, id, slot, p).await?;
    }
    to_period_end(&e, id).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    let m = market(&e, id).await?;
    assert_eq!(m["settled_average_micros"], "4000000", "mean of the three filled slots");
    assert_eq!(m["status"], json!({"Settled": {"long_payout_micros": 500_000}}));
    Ok(())
}

#[tokio::test]
async fn too_few_slots_waits_then_voids_then_second_settle_refused() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    mint(&e, &e.alice, id, 10 * ONE).await?.into_result()?;
    observe_in_slot(&e, id, 0, 4_000_000).await?;
    to_period_end(&e, id).await?;
    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_TOO_FEW_OBSERVATIONS"), "{:?}", r.failures());
    let m = market(&e, id).await?;
    advance_to(&e.worker, n(&m["void_after"])).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(market(&e, id).await?["status"], "Void");
    let r = settle(&e, &e.alice, id).await?;
    assert!(fails_with(&r, "ERR_ALREADY_SETTLED"), "{:?}", r.failures());
    // Void pays 500_000 per claim on each side: 10 Long + 10 Short -> 10 fUSD.
    let a0 = fusd_balance(&e, &e.alice).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 10 * ONE);
    Ok(())
}

// ---------------- Rolling ----------------

#[tokio::test]
async fn roll_before_previous_done_fails_and_roll_after_settle_uses_the_average() -> anyhow::Result<()> {
    let e = setup().await?;
    let r = roll(&e, &e.bob, "DEMO").await?;
    assert!(fails_with(&r, "ERR_NO_PREVIOUS"), "{:?}", r.failures());
    let id = open_demo(&e).await?;
    let r = roll(&e, &e.bob, "DEMO").await?;
    assert!(fails_with(&r, "ERR_PREVIOUS_NOT_DONE"), "{:?}", r.failures());
    for (slot, p) in [(0u64, 3_980_000u64), (1, 3_980_000), (2, 3_980_000)] {
        observe_in_slot(&e, id, slot, p).await?;
    }
    to_period_end(&e, id).await?;
    settle(&e, &e.alice, id).await?.into_result()?;

    // Any account can roll; the band is 0.6 / 1.4 of 3.98, rounded down to 10 cents.
    let r = roll(&e, &e.bob, "DEMO").await?;
    assert!(r.is_success(), "{:?}", r.failures());
    let next: u64 = r.json::<String>()?.parse()?;
    assert_eq!(next, id + 1);
    let m = market(&e, next).await?;
    assert_eq!(m["floor_micros"], "2300000");
    assert_eq!(m["cap_micros"], "5500000");
    assert_eq!(m["created_by"], e.bob.id().to_string());
    assert_eq!(m["status"], "Auction");
    let r = roll(&e, &e.alice, "DEMO").await?;
    assert!(fails_with(&r, "ERR_PREVIOUS_NOT_DONE"), "only one open market per template: {:?}", r.failures());
    Ok(())
}

#[tokio::test]
async fn roll_after_void_reuses_the_band_with_no_full_access_key() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = create(&e, "DEMO", 2_500_000, 5_000_000).await?;
    let m = market(&e, id).await?;
    advance_to(&e.worker, n(&m["auction_reveal_end"])).await?;
    clear(&e, id).await?.into_result()?;
    advance_to(&e.worker, n(&m["void_after"])).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(market(&e, id).await?["status"], "Void");

    // Delete the contract account's only full-access key.
    let keys = e.flat.as_account().view_access_keys().await?;
    assert_eq!(keys.len(), 1);
    e.flat.as_account().batch(e.flat.id()).delete_key(keys[0].public_key.clone()).transact().await?.into_result()?;
    assert!(e.flat.as_account().view_access_keys().await?.is_empty());

    let r = roll(&e, &e.bob, "DEMO").await?;
    assert!(r.is_success(), "{:?}", r.failures());
    let next: u64 = r.json::<String>()?.parse()?;
    let m = market(&e, next).await?;
    assert_eq!(m["floor_micros"], "2500000", "void market: band reused");
    assert_eq!(m["cap_micros"], "5000000");
    Ok(())
}

// ---------------- Sealed opening auction ----------------

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

async fn refund_balance(e: &Env, who: &Account) -> anyhow::Result<u128> {
    let v: String = e.flat.view("refund_balance_of").args_json(json!({"account_id": who.id()})).await?.json()?;
    Ok(v.parse()?)
}

async fn withdraw(e: &Env, who: &Account) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), "withdraw_refund").gas(Gas::from_tgas(50)).transact().await?)
}

async fn bids(e: &Env, market: u64) -> anyhow::Result<Vec<Value>> {
    Ok(e.flat.view("get_bids").args_json(json!({"market_id": market.to_string(), "limit": 200})).await?.json()?)
}

/// A receipt in the transaction failed with `err` (the outer ft_transfer_call may still succeed
/// because NEP-141 `ft_resolve_transfer` refunds the sender).
fn receipt_failed_with(res: &ExecutionFinalResult, err: &str) -> bool {
    format!("{:?}", res.failures()).contains(err)
}

/// The WEEKLY test market (long auction windows) and its two deadlines.
async fn auction_market(e: &Env) -> anyhow::Result<(u64, u64, u64)> {
    let id = create(e, "WEEKLY", FLOOR, CAP).await?;
    let m = market(e, id).await?;
    Ok((id, n(&m["auction_commit_end"]), n(&m["auction_reveal_end"])))
}

#[tokio::test]
async fn auction_two_bid_match() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    let long = TestBid::new("Long", 10, 60, "alice-salt-1", ONE);
    let short = TestBid::new("Short", 10, 45, "bob-salt-1", 0);
    let view_hash: String = e.flat.view("bid_hash").args_json(reveal_args(id, &long)).await?.json()?;
    assert_eq!(view_hash, bid_hash(id, &long));
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    assert_eq!(market(&e, id).await?["bid_count"], 2);
    assert!(bids(&e, id).await?[0]["revealed"].is_null());
    advance_to(&e.worker, commit_end).await?;
    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, reveal_end).await?;
    clear(&e, id).await?.into_result()?;
    let m = market(&e, id).await?;
    assert_eq!(m["status"], "Open");
    assert_eq!(m["opening_price_cents"], 55);
    assert_eq!(m["pairs"], (10 * ONE).to_string());
    assert_eq!(claims(&e, &e.alice, id, "Long").await?, 10 * ONE);
    assert_eq!(claims(&e, &e.bob, id, "Short").await?, 10 * ONE);
    assert_eq!(refund_balance(&e, &e.alice).await?, 1_500_000);
    assert_eq!(refund_balance(&e, &e.bob).await?, 0);
    withdraw(&e, &e.alice).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, a0 - 5_500_000);
    assert_eq!(fusd_balance(&e, &e.bob).await?, b0 - 4_500_000);
    let r = withdraw(&e, &e.alice).await?;
    assert!(fails_with(&r, "ERR_NOTHING_TO_REFUND"), "{:?}", r.failures());
    let r = clear(&e, id).await?;
    assert!(fails_with(&r, "ERR_NOT_IN_AUCTION"), "{:?}", r.failures());
    Ok(())
}

#[tokio::test]
async fn auction_tie_break() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
    let long = TestBid::new("Long", 5, 70, "l", 0);
    let short = TestBid::new("Short", 5, 70, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, commit_end).await?;
    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, reveal_end).await?;
    clear(&e, id).await?.into_result()?;
    assert_eq!(market(&e, id).await?["opening_price_cents"], 50);
    assert_eq!(refund_balance(&e, &e.alice).await?, ONE);
    assert_eq!(refund_balance(&e, &e.bob).await?, ONE);
    Ok(())
}

#[tokio::test]
async fn auction_partial_fill() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
    let l0 = TestBid::new("Long", 5, 60, "l0", 0);
    let l1 = TestBid::new("Long", 5, 55, "l1", 0);
    let l2 = TestBid::new("Long", 5, 60, "l2", 0);
    let s = TestBid::new("Short", 7, 50, "s", 0);
    commit(&e, &e.alice, id, &l0).await?.into_result()?;
    commit(&e, &e.bob, id, &l1).await?.into_result()?;
    commit(&e, &e.alice, id, &l2).await?.into_result()?;
    commit(&e, &e.bob, id, &s).await?.into_result()?;
    advance_to(&e.worker, commit_end).await?;
    for (who, b) in [(&e.alice, &l0), (&e.bob, &l1), (&e.alice, &l2), (&e.bob, &s)] {
        reveal(&e, who, id, b).await?.into_result()?;
    }
    advance_to(&e.worker, reveal_end).await?;
    clear(&e, id).await?.into_result()?;
    let b = bids(&e, id).await?;
    assert_eq!(market(&e, id).await?["opening_price_cents"], 50);
    assert_eq!(b[0]["filled_qty"], "5");
    assert_eq!(b[1]["filled_qty"], "0");
    assert_eq!(b[2]["filled_qty"], "2");
    assert_eq!(b[3]["filled_qty"], "7");
    assert_eq!(refund_balance(&e, &e.alice).await?, 2_500_000);
    assert_eq!(refund_balance(&e, &e.bob).await?, 2_750_000);
    Ok(())
}

#[tokio::test]
async fn auction_unrevealed_bid_refunded() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
    let a0 = fusd_balance(&e, &e.alice).await?;
    let long = TestBid::new("Long", 10, 60, "never-revealed", 250_000);
    let short = TestBid::new("Short", 10, 45, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    commit(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, commit_end).await?;
    reveal(&e, &e.bob, id, &short).await?.into_result()?;
    advance_to(&e.worker, reveal_end).await?;
    clear(&e, id).await?.into_result()?;
    assert!(market(&e, id).await?["opening_price_cents"].is_null());
    assert_eq!(refund_balance(&e, &e.alice).await?, long.escrow);
    assert_eq!(refund_balance(&e, &e.bob).await?, short.escrow);
    withdraw(&e, &e.alice).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, a0);
    Ok(())
}

#[tokio::test]
async fn auction_wrong_salt_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, _) = auction_market(&e).await?;
    let long = TestBid::new("Long", 10, 60, "right-salt", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    let under = TestBid { escrow: 1_000_000, ..TestBid::new("Long", 10, 60, "under", 0) };
    commit(&e, &e.alice, id, &under).await?.into_result()?;
    advance_to(&e.worker, commit_end).await?;
    let wrong = TestBid { salt: "wrong-salt".to_string(), ..long.clone() };
    let r = reveal(&e, &e.alice, id, &wrong).await?;
    assert!(fails_with(&r, "ERR_BAD_REVEAL"), "{:?}", r.failures());
    let r = reveal(&e, &e.bob, id, &long).await?;
    assert!(fails_with(&r, "ERR_BAD_REVEAL"), "{:?}", r.failures());
    let r = reveal(&e, &e.alice, id, &under).await?;
    assert!(fails_with(&r, "ERR_ESCROW_TOO_LOW"), "{:?}", r.failures());
    reveal(&e, &e.alice, id, &long).await?.into_result()?;
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_ALREADY_REVEALED"), "{:?}", r.failures());
    Ok(())
}

#[tokio::test]
async fn auction_late_reveal_rejected() -> anyhow::Result<()> {
    let e = setup().await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
    let long = TestBid::new("Long", 10, 60, "s", 0);
    commit(&e, &e.alice, id, &long).await?.into_result()?;
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_REVEAL_NOT_OPEN"), "{:?}", r.failures());
    let r = clear(&e, id).await?;
    assert!(fails_with(&r, "ERR_REVEAL_NOT_ENDED"), "{:?}", r.failures());
    advance_to(&e.worker, commit_end).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    let r = commit(&e, &e.bob, id, &TestBid::new("Short", 10, 45, "late", 0)).await?;
    assert!(receipt_failed_with(&r, "ERR_COMMIT_ENDED"), "{:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.bob).await?, b0);
    advance_to(&e.worker, reveal_end).await?;
    let r = reveal(&e, &e.alice, id, &long).await?;
    assert!(fails_with(&r, "ERR_REVEAL_ENDED"), "{:?}", r.failures());
    clear(&e, id).await?.into_result()?;
    assert_eq!(refund_balance(&e, &e.alice).await?, long.escrow);
    Ok(())
}

#[tokio::test]
async fn auction_200_bids_clear_within_gas() -> anyhow::Result<()> {
    let e = setup_with(DEMO, Tpl(900, 600, 600, 60, 3, 300)).await?;
    let (id, commit_end, reveal_end) = auction_market(&e).await?;
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
    let r = commit(&e, &e.alice, id, &TestBid::new("Long", 1, 50, "extra", 0)).await?;
    assert!(receipt_failed_with(&r, "ERR_TOO_MANY_BIDS"), "{:?}", r.failures());
    advance_to(&e.worker, commit_end).await?;
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
    advance_to(&e.worker, reveal_end).await?;
    let r = clear(&e, id).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    // `outcome()` is only the transaction-to-receipt step; the function runs in the receipt
    // executed by the flatrate account.
    let clear_gas: u64 =
        r.receipt_outcomes().iter().filter(|o| o.executor_id == *e.flat.id()).map(|o| o.gas_burnt.as_gas()).sum();
    println!(
        "CLEAR_200_GAS clear_auction receipt gas_burnt = {} ({:.2} TGas); transaction total = {} ({:.2} TGas)",
        clear_gas,
        clear_gas as f64 / 1e12,
        r.total_gas_burnt.as_gas(),
        r.total_gas_burnt.as_gas() as f64 / 1e12
    );
    assert!(clear_gas > 1_000_000_000_000, "measured the function receipt, not the conversion step");
    assert!(clear_gas < 300_000_000_000_000);
    let m = market(&e, id).await?;
    let pairs: u128 = m["pairs"].as_str().unwrap().parse()?;
    assert!(pairs > 0);
    let ra = refund_balance(&e, &e.alice).await?;
    let rb = refund_balance(&e, &e.bob).await?;
    let escrow: u128 = longs.iter().chain(&shorts).map(|b| b.escrow).sum();
    assert_eq!(pairs + ra + rb, escrow);
    assert_eq!(fusd_balance(&e, &e.flat.as_account().clone()).await?, escrow);
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
    Ok(who.call(e.flat.id(), method).args_json(json!({"token_id": token})).gas(Gas::from_tgas(100)).transact().await?)
}

async fn nft_token(e: &Env, token: &str) -> anyhow::Result<Value> {
    Ok(e.flat.view("nft_token").args_json(json!({"token_id": token})).await?.json()?)
}

#[tokio::test]
async fn note_wrap_transfer_new_owner_redeems_old_owner_cannot() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    mint(&e, &e.alice, id, 100 * ONE).await?.into_result()?;
    let r = wrap(&e, &e.alice, id, "Long", 40 * ONE).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert!(has_log(&r, "nft_mint"), "{:?}", r.logs());
    let tok = nft_token(&e, "1000").await?;
    assert_eq!(tok["owner_id"], e.alice.id().to_string());
    let title = tok["metadata"]["title"].as_str().unwrap().to_string();
    assert!(title.starts_with("Cover Note #1000: 40 Long, H100, period ending "), "{title}");
    let meta: Value = e.flat.view("nft_metadata").await?.json()?;
    assert_eq!(meta["spec"], "nft-1.0.0");
    e.alice
        .call(e.flat.id(), "nft_transfer")
        .args_json(json!({"receiver_id": e.bob.id(), "token_id": "1000"}))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;
    let r = note_call(&e, &e.bob, "redeem_note", "1000").await?;
    assert!(fails_with(&r, "ERR_NOT_SETTLED"), "{:?}", r.failures());
    for (slot, p) in [(0u64, 3_980_000u64), (1, 3_990_000), (2, 4_000_000)] {
        observe_in_slot(&e, id, slot, p).await?;
    }
    to_period_end(&e, id).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    let r = note_call(&e, &e.alice, "redeem_note", "1000").await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());
    let b0 = fusd_balance(&e, &e.bob).await?;
    let r = note_call(&e, &e.bob, "redeem_note", "1000").await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(fusd_balance(&e, &e.bob).await? - b0, 19_875_000);
    assert!(nft_token(&e, "1000").await?.is_null());
    let a0 = fusd_balance(&e, &e.alice).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 60 * 496_875 + 100 * 503_125);
    assert_eq!(market(&e, id).await?["collateral"], "0");
    Ok(())
}

#[tokio::test]
async fn note_unwrap_before_settle() -> anyhow::Result<()> {
    let e = setup().await?;
    let id = open_demo(&e).await?;
    mint(&e, &e.alice, id, 10 * ONE).await?.into_result()?;
    wrap(&e, &e.alice, id, "Short", 4 * ONE).await?.into_result()?;
    wrap(&e, &e.alice, id, "Short", 3_500_000).await?.into_result()?;
    let title = nft_token(&e, "1001").await?["metadata"]["title"].as_str().unwrap().to_string();
    assert!(title.starts_with("Cover Note #1001: 3.5 Short"), "{title}");
    let r = note_call(&e, &e.bob, "unwrap_note", "1000").await?;
    assert!(fails_with(&r, "ERR_NOT_OWNER"), "{:?}", r.failures());
    note_call(&e, &e.alice, "unwrap_note", "1000").await?.into_result()?;
    assert_eq!(claims(&e, &e.alice, id, "Short").await?, 6_500_000);
    let r = note_call(&e, &e.alice, "unwrap_note", "1000").await?;
    assert!(fails_with(&r, "ERR_NO_NOTE"), "{:?}", r.failures());
    for (slot, p) in [(0u64, 4_000_000u64), (1, 4_000_000), (2, 4_000_000)] {
        observe_in_slot(&e, id, slot, p).await?;
    }
    to_period_end(&e, id).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    let r = note_call(&e, &e.alice, "unwrap_note", "1001").await?;
    assert!(fails_with(&r, "ERR_ALREADY_SETTLED"), "{:?}", r.failures());
    let a0 = fusd_balance(&e, &e.alice).await?;
    note_call(&e, &e.alice, "redeem_note", "1001").await?.into_result()?;
    assert_eq!(fusd_balance(&e, &e.alice).await? - a0, 1_750_000);
    Ok(())
}

// ---------------- Fresh start over the previously deployed contract ----------------

#[tokio::test]
async fn reset_over_deployed_state_starts_empty() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let owner = root.create_subaccount("owner").initial_balance(NearToken::from_near(50)).transact().await?.into_result()?;
    let fusd = root
        .create_subaccount("fusd")
        .initial_balance(NearToken::from_near(20))
        .transact()
        .await?
        .into_result()?
        .deploy(&wasm("fusd"))
        .await?
        .into_result()?;
    fusd.call("new").transact().await?.into_result()?;
    // The exact wasm that was live on testnet before this version (sha256 c8fd8da6...).
    let old = std::fs::read(format!("{}/tests/res/flatrate_phase4_deployed.wasm", env!("CARGO_MANIFEST_DIR")))?;
    let flat_acc = root.create_subaccount("flatrate").initial_balance(NearToken::from_near(30)).transact().await?.into_result()?;
    let flat = flat_acc.deploy(&old).await?.into_result()?;
    flat.call("new")
        .args_json(json!({
            "owner_id": owner.id(), "fusd_id": fusd.id(), "outlayer_id": "outlayer.test.near",
            "oracle_project_id": "x/flatrate-oracle", "oracle_version_key": "v1", "oracle_sources": SOURCES,
            "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
        }))
        .transact()
        .await?
        .into_result()?;
    let t0 = worker.view_block().await?.timestamp();
    owner
        .call(flat.id(), "create_market")
        .args_json(json!({"index": "H100_HOURLY", "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string(),
            "auction_commit_end": (t0 + 600 * SEC).to_string(), "auction_reveal_end": (t0 + 1200 * SEC).to_string(),
            "period_start": (t0 + 1200 * SEC).to_string(), "period_end": (t0 + 1800 * SEC).to_string(),
            "min_observations": 3, "void_after": (t0 + 2400 * SEC).to_string()}))
        .transact()
        .await?
        .into_result()?;

    // Deploy the new code with `reset` as the init call; the old state is ignored.
    let r = flat_acc
        .batch(flat.id())
        .deploy(&wasm("flatrate"))
        .call(Function::new("reset").args_json(init_args(&owner, &fusd, "outlayer.test.near", DEMO, WEEKLY)).gas(Gas::from_tgas(100)))
        .transact()
        .await?;
    assert!(r.is_success(), "{:?}", r.failures());
    let r = owner.call(flat.id(), "reset").args_json(init_args(&owner, &fusd, "outlayer.test.near", DEMO, WEEKLY)).transact().await?;
    assert!(!r.is_success(), "reset is private");
    let listed: Vec<Value> = flat.view("list_markets").args_json(json!({})).await?.json()?;
    assert!(listed.is_empty(), "no market from the old state is visible");
    let cfg: Value = flat.view("get_config").await?.json()?;
    assert_eq!(cfg["next_market_id"], "10");
    let id: String = owner
        .call(flat.id(), "create_market")
        .args_json(json!({"template": "DEMO", "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string()}))
        .transact()
        .await?
        .into_result()?
        .json()?;
    assert_eq!(id, "10");
    Ok(())
}

// ---------------- Scripted demo: replay testnet market 2 from fixtures ----------------

/// `make demo-scripted`. Replays testnet market 2 from `fixtures/demo/market-2.json` (recorded
/// from public RPC by scripts/record-demo-fixture.mjs) against the sandbox: same floor and cap,
/// the same revealed bids, and each recorded OutLayer output fed through mock-outlayer, one per
/// slot. The result must equal what testnet recorded: opening price, pairs, every observed
/// price, the Long payout and the redemption. No network, no keys; the same result every run.
#[tokio::test]
async fn scripted_demo_replays_market_2() -> anyhow::Result<()> {
    let path = format!("{}/../../fixtures/demo/market-2.json", env!("CARGO_MANIFEST_DIR"));
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let recorded = &fx["market"];
    let floor = n(&recorded["floor_micros"]);
    let cap = n(&recorded["cap_micros"]);
    println!("SCRIPTED DEMO (fixture recorded {} from {})", fx["recorded_at"], fx["source"]);

    let e = setup().await?;
    let id = create(&e, "DEMO", floor, cap).await?;
    let m = market(&e, id).await?;
    println!("  1. market {id} created: floor {floor}, cap {cap}");
    let bids_fx: Vec<TestBid> = fx["bids"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let r = &b["revealed"];
            let side = if r["side"] == "Long" { "Long" } else { "Short" };
            let mut t = TestBid::new(side, n(&r["qty"]), r["limit_cents"].as_u64().unwrap() as u8, &format!("replay-{i}"), 0);
            t.escrow = b["escrow"].as_str().unwrap().parse().unwrap();
            t
        })
        .collect();
    for b in &bids_fx {
        commit(&e, if b.side == "Long" { &e.alice } else { &e.bob }, id, b).await?.into_result()?;
    }
    println!("  2. {} sealed bids committed", bids_fx.len());
    advance_to(&e.worker, n(&m["auction_commit_end"])).await?;
    for b in &bids_fx {
        reveal(&e, if b.side == "Long" { &e.alice } else { &e.bob }, id, b).await?.into_result()?;
    }
    println!("  3. bids revealed: {}", bids_fx.iter().map(|b| format!("{} {} @ {}c", b.side, b.qty, b.limit)).collect::<Vec<_>>().join(", "));
    advance_to(&e.worker, n(&m["auction_reveal_end"])).await?;
    clear(&e, id).await?.into_result()?;
    let m2 = market(&e, id).await?;
    assert_eq!(m2["opening_price_cents"], recorded["opening_price_cents"], "opening price matches testnet");
    assert_eq!(m2["pairs"], recorded["pairs"], "pairs match testnet");
    println!("  4. auction cleared at {}c, {} pairs (testnet: {}c, {})", m2["opening_price_cents"], m2["pairs"], recorded["opening_price_cents"], recorded["pairs"]);

    let outputs: Vec<&Value> = fx["steps"].as_array().unwrap().iter().filter_map(|s| s.get("oracle").filter(|o| !o.is_null())).collect();
    for (i, out) in outputs.iter().enumerate() {
        to_slot(&e, id, i as u64).await?;
        let r = observe(&e, id, "Respond", Some(out.to_string())).await?;
        assert!(has_log(&r, "observation_recorded"), "{:?}", r.logs());
        println!(
            "  5.{} slot {i}: median {} from {} sources, OutLayer request {}",
            i + 1,
            out["price_micros"],
            out["sources"].as_array().unwrap().iter().filter(|s| !s["price_micros"].is_null()).count(),
            out["request_id"]
        );
    }
    let ours: Vec<u64> = market(&e, id).await?["observations"].as_array().unwrap().iter().map(|o| n(&o["price_micros"])).collect();
    let theirs: Vec<u64> = recorded["observations"].as_array().unwrap().iter().map(|o| n(&o["price_micros"])).collect();
    assert_eq!(ours, theirs, "observed prices match testnet");

    to_period_end(&e, id).await?;
    settle(&e, &e.alice, id).await?.into_result()?;
    let status = market(&e, id).await?["status"].clone();
    assert_eq!(status, recorded["status"], "settlement matches testnet");
    println!("  6. settled: {status} (testnet: {})", recorded["status"]);
    let a0 = fusd_balance(&e, &e.alice).await?;
    let b0 = fusd_balance(&e, &e.bob).await?;
    redeem(&e, &e.alice, id).await?.into_result()?;
    redeem(&e, &e.bob, id).await?.into_result()?;
    let (ga, gb) = (fusd_balance(&e, &e.alice).await? - a0, fusd_balance(&e, &e.bob).await? - b0);
    let lp = recorded["status"]["Settled"]["long_payout_micros"].as_u64().unwrap() as u128;
    let pairs: u128 = recorded["pairs"].as_str().unwrap().parse()?;
    assert_eq!(ga, pairs * lp / ONE);
    assert_eq!(gb, pairs * (ONE - lp) / ONE);
    assert_eq!(market(&e, id).await?["collateral"], "0");
    println!("  7. redeemed: Long holder {ga}, Short holder {gb} fUSD units; collateral 0");
    println!("SCRIPTED DEMO PASS: sandbox replay equals testnet market 2");
    Ok(())
}

/// Extra 2: the same contract deployed for another index. The index is fixed at init, goes into
/// every market's oracle input, and defaults to H100_HOURLY when the init omits it.
#[tokio::test]
async fn index_is_fixed_at_init_and_defaults_to_h100() -> anyhow::Result<()> {
    let e = setup().await?;
    let cfg: Value = e.flat.view("get_config").await?.json()?;
    assert_eq!(cfg["index"], "H100_HOURLY");

    let tokens = e
        .worker
        .root_account()?
        .create_subaccount("tokens")
        .initial_balance(NearToken::from_near(30))
        .transact()
        .await?
        .into_result()?
        .deploy(&wasm("flatrate"))
        .await?
        .into_result()?;
    let mut args = init_args(&e.owner, &e.fusd, e.mock.id().as_str(), DEMO, WEEKLY);
    args["index"] = json!("TOKEN_PRICE");
    args["oracle_sources"] = json!(["deepinfra", "parasail", "groq", "sambanova", "coreweave"]);
    tokens.call("new").args_json(args).transact().await?.into_result()?;
    let cfg: Value = tokens.view("get_config").await?.json()?;
    assert_eq!(cfg["index"], "TOKEN_PRICE");

    e.owner
        .call(tokens.id(), "create_market")
        .args_json(json!({"template": "DEMO", "floor_micros": "400000", "cap_micros": "1000000"}))
        .transact()
        .await?
        .into_result()?;
    let m: Value = tokens.view("get_market").args_json(json!({"market_id": "10"})).await?.json()?;
    assert_eq!(m["index"], "TOKEN_PRICE");
    let input: Value = serde_json::from_str(m["oracle_input"].as_str().unwrap())?;
    assert_eq!(input["index"], "TOKEN_PRICE");
    assert_eq!(input["sources"][2], "groq");
    Ok(())
}
