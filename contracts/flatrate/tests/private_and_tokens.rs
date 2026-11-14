//! Sandbox tests for the two extras added on 2026-11-14:
//! - private auction: bids sealed to an OutLayer enclave key and cleared by the enclave
//!   (`auction-enclave/`); here `mock-outlayer` stands in for OutLayer and returns the enclave's
//!   output, so these tests check the contract's side: commits, gates, checks on the result,
//!   refunds and the abandon path. The enclave's own decryption and clearing are tested natively
//!   in `auction-enclave` (`cargo test`).
//! - claim tokens: `claim_transfer_call` into `contracts/claim-token`, NEP-141 transfer, unwrap.
//! Needs `make build` first (reads target/wasm/*.wasm).

use near_workspaces::network::Sandbox;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{Gas, NearToken};
use near_workspaces::{Account, Contract, Worker};
use serde_json::{json, Value};

const ONE: u128 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const FLOOR: u64 = 2_400_000;
const CAP: u64 = 5_600_000;
const PUBKEY: &str = "04574ac668de84eff408565a477722f52a548794d7f9013060621be27dc4ba0e";

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

fn tpl(commit: u64, reveal: u64, period: u64, slot: u64, min: u32, void_after: u64) -> Value {
    json!({
        "commit_ns": (commit * SEC).to_string(), "reveal_ns": (reveal * SEC).to_string(),
        "period_ns": (period * SEC).to_string(), "slot_ns": (slot * SEC).to_string(),
        "min_filled_slots": min, "void_after_ns": (void_after * SEC).to_string(),
    })
}

async fn setup(private: bool) -> anyhow::Result<Env> {
    let worker = near_workspaces::sandbox().await?;
    let root = worker.root_account()?;
    let mk = |name: &'static str, near: u128| {
        let root = root.clone();
        async move {
            root.create_subaccount(name).initial_balance(NearToken::from_near(near)).transact().await?.into_result().map_err(anyhow::Error::from)
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
    let enclave = if private {
        json!({"project_id": format!("{}/flatrate-auction", owner.id()), "version_key": "mock-enclave-sha256"})
    } else {
        Value::Null
    };
    flat.call("new")
        .args_json(json!({
            "owner_id": owner.id(), "fusd_id": fusd.id(), "outlayer_id": mock.id(),
            "oracle_project_id": format!("{}/flatrate-oracle", owner.id()), "oracle_version_key": "mock-version-sha256",
            "oracle_sources": ["akash", "vast", "runpod", "shadeform", "verda"],
            "resource_limits": {"max_instructions": "1000000000", "max_memory_mb": 128, "max_execution_seconds": "30"},
            // DEMO: 20 s commits, 20 s reveals, 180 s period of 30 s slots, void 60 s after.
            "demo_template": tpl(20, 20, 180, 30, 3, 60),
            "weekly_template": tpl(60, 60, 600, 60, 3, 300),
            "observation_deposit": "10000000000000000000000",
            "enclave": enclave,
        }))
        .transact()
        .await?
        .into_result()?;
    owner.call(fusd.id(), "storage_deposit").args_json(json!({"account_id": flat.id()})).deposit(NearToken::from_millinear(10)).transact().await?.into_result()?;
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

async fn config(e: &Env) -> anyhow::Result<Value> {
    Ok(e.flat.view("get_config").args_json(json!({})).await?.json()?)
}

async fn create(e: &Env) -> anyhow::Result<u64> {
    let id: String = e
        .owner
        .call(e.flat.id(), "create_market")
        .args_json(json!({"template": "DEMO", "floor_micros": FLOOR.to_string(), "cap_micros": CAP.to_string()}))
        .transact()
        .await?
        .into_result()?
        .json()?;
    Ok(id.parse()?)
}

async fn set_mock(e: &Env, output: Option<String>) -> anyhow::Result<()> {
    let mode = if output.is_some() { "Respond" } else { "Null" };
    e.owner.call(e.mock.id(), "mock_set_next").args_json(json!({"mode": mode, "output": output})).transact().await?.into_result()?;
    Ok(())
}

fn fails_with(res: &ExecutionFinalResult, err: &str) -> bool {
    !res.is_success() && format!("{:?}", res.failures()).contains(err)
}

fn has_log(res: &ExecutionFinalResult, needle: &str) -> bool {
    res.logs().iter().any(|l| l.contains(needle))
}

async fn call(who: &Account, e: &Env, method: &str, args: Value) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who.call(e.flat.id(), method).args_json(args).gas(Gas::from_tgas(300)).transact().await?)
}

async fn ft_call(e: &Env, who: &Account, amount: u128, msg: Value) -> anyhow::Result<ExecutionFinalResult> {
    Ok(who
        .call(e.fusd.id(), "ft_transfer_call")
        .args_json(json!({"receiver_id": e.flat.id(), "amount": amount.to_string(), "msg": msg.to_string()}))
        .deposit(NearToken::from_yoctonear(1))
        .gas(Gas::from_tgas(100))
        .transact()
        .await?)
}

async fn commit_sealed(e: &Env, who: &Account, market: u64, escrow: u128, ct: &str) -> anyhow::Result<ExecutionFinalResult> {
    ft_call(e, who, escrow, json!({"action": "commit_sealed", "market_id": market.to_string(), "ciphertext": ct})).await
}

async fn fusd_balance(e: &Env, who: &Account) -> anyhow::Result<u128> {
    let v: String = e.fusd.view("ft_balance_of").args_json(json!({"account_id": who.id()})).await?.json()?;
    Ok(v.parse()?)
}

async fn refund_of(e: &Env, who: &Account) -> anyhow::Result<u128> {
    let v: String = e.flat.view("refund_balance_of").args_json(json!({"account_id": who.id()})).await?.json()?;
    Ok(v.parse()?)
}

async fn claims(e: &Env, who: &str, market: u64, side: &str) -> anyhow::Result<u128> {
    let v: String = e.flat.view("claim_balance_of").args_json(json!({"account_id": who, "market_id": market.to_string(), "side": side})).await?.json()?;
    Ok(v.parse()?)
}

fn clear_output(market: u64, price: Option<u8>, matched: u64, fills: Value) -> String {
    json!({"op": "clear", "market_id": market.to_string(), "price_cents": price, "matched": matched, "fills": fills,
           "valid": 2, "invalid": 0, "request_id": "mock-request"})
    .to_string()
}

/// Private contract with its enclave key set through the mock.
async fn private_with_key() -> anyhow::Result<Env> {
    let e = setup(true).await?;
    set_mock(&e, Some(json!({"op": "pubkey", "x25519_pubkey": PUBKEY, "request_id": "mock"}).to_string())).await?;
    let r = call(&e.alice, &e, "request_enclave_key", json!({})).await?;
    assert!(r.is_success() && has_log(&r, "enclave_key_set"), "{:?} {:?}", r.failures(), r.logs());
    Ok(e)
}

// ---------------- private auction ----------------

#[tokio::test]
async fn private_auction_key_is_set_once_and_public_paths_are_closed() -> anyhow::Result<()> {
    let e = private_with_key().await?;
    assert_eq!(config(&e).await?["enclave_pubkey"], json!(PUBKEY));
    assert!(fails_with(&call(&e.bob, &e, "request_enclave_key", json!({})).await?, "ERR_KEY_ALREADY_SET"));

    let id = create(&e).await?;
    // A plain hash commit and a reveal are refused on a private-auction contract.
    let r = ft_call(&e, &e.alice, 5 * ONE, json!({"action": "commit_bid", "market_id": id.to_string(), "hash": "00".repeat(32)})).await?;
    assert!(has_log(&r, "ERR_PRIVATE_AUCTION") || format!("{:?}", r.receipt_failures()).contains("ERR_PRIVATE_AUCTION"), "{:?}", r.logs());
    assert_eq!(fusd_balance(&e, &e.alice).await?, 1_000 * ONE, "a refused commit returns the fUSD");
    let r = call(&e.alice, &e, "reveal_bid", json!({"market_id": id.to_string(), "side": "Long", "qty": "1", "limit_cents": 50, "salt": "x"})).await?;
    assert!(fails_with(&r, "ERR_PRIVATE_AUCTION"));
    Ok(())
}

#[tokio::test]
async fn sealed_commit_needs_the_key() -> anyhow::Result<()> {
    let e = setup(true).await?;
    let id = create(&e).await?;
    commit_sealed(&e, &e.alice, id, 6 * ONE, "c2VhbGVk").await?;
    assert_eq!(fusd_balance(&e, &e.alice).await?, 1_000 * ONE, "no key yet: the commit is refused and refunded");
    assert_eq!(market(&e, id).await?["bid_count"], json!(0));
    Ok(())
}

#[tokio::test]
async fn private_auction_clears_through_the_enclave() -> anyhow::Result<()> {
    let e = private_with_key().await?;
    let id = create(&e).await?;
    assert!(commit_sealed(&e, &e.alice, id, 6 * ONE, "YWxpY2Utc2VhbGVk").await?.is_success());
    assert!(commit_sealed(&e, &e.bob, id, 5 * ONE, "Ym9iLXNlYWxlZA==").await?.is_success());
    let bids: Value = e.flat.view("get_bids").args_json(json!({"market_id": id.to_string()})).await?.json()?;
    assert_eq!(bids[0]["ciphertext"], json!("YWxpY2Utc2VhbGVk"));
    assert_eq!(bids[0]["revealed"], Value::Null);

    // Too early: the commit window is still open.
    assert!(fails_with(&call(&e.alice, &e, "clear_auction", json!({"market_id": id.to_string()})).await?, "ERR_COMMIT_NOT_ENDED"));
    let m = market(&e, id).await?;
    advance_to(&e.worker, n(&m["auction_commit_end"])).await?;

    // The enclave opened both bids: alice Long 10 @ <= 55c, bob Short 10 @ <= 50c -> 50c, 10 pairs.
    set_mock(&e, Some(clear_output(id, Some(50), 10, json!([
        {"index": 0, "side": "Long", "filled_qty": 10}, {"index": 1, "side": "Short", "filled_qty": 10}
    ])))).await?;
    let r = call(&e.bob, &e, "clear_auction", json!({"market_id": id.to_string()})).await?;
    assert!(r.is_success() && has_log(&r, "auction_cleared"), "{:?} {:?}", r.failures(), r.logs());
    // The contract sent the ciphertexts, not plaintext, to the enclave project.
    let args: String = e.mock.view("mock_last_args").await?.json()?;
    assert!(args.contains("YWxpY2Utc2VhbGVk") && args.contains("flatrate-auction") && args.contains("mock-enclave-sha256"));

    let m = market(&e, id).await?;
    assert_eq!(m["status"], json!("Open"));
    assert_eq!(m["opening_price_cents"], json!(50));
    assert_eq!(n(&m["pairs"]), 10 * ONE as u64);
    assert_eq!(claims(&e, e.alice.id().as_str(), id, "Long").await?, 10 * ONE);
    assert_eq!(claims(&e, e.bob.id().as_str(), id, "Short").await?, 10 * ONE);
    // Alice escrowed 6, paid 10 x 50c = 5 -> refund 1; bob escrowed 5, paid 5 -> refund 0.
    assert_eq!(refund_of(&e, &e.alice).await?, ONE);
    assert_eq!(refund_of(&e, &e.bob).await?, 0);
    // Cleared once: a second clear is refused.
    assert!(fails_with(&call(&e.bob, &e, "clear_auction", json!({"market_id": id.to_string()})).await?, "ERR_NOT_IN_AUCTION"));
    Ok(())
}

#[tokio::test]
async fn bad_enclave_results_record_nothing_and_clearing_can_retry() -> anyhow::Result<()> {
    let e = private_with_key().await?;
    let id = create(&e).await?;
    commit_sealed(&e, &e.alice, id, 6 * ONE, "YQ==").await?.into_result()?;
    commit_sealed(&e, &e.bob, id, 5 * ONE, "Yg==").await?.into_result()?;
    advance_to(&e.worker, n(&market(&e, id).await?["auction_commit_end"])).await?;

    let cases = [
        // Bob pays 10 x 60c = 6 > escrow 5.
        (clear_output(id, Some(40), 10, json!([{"index": 0, "side": "Long", "filled_qty": 10}, {"index": 1, "side": "Short", "filled_qty": 10}])), "fill_exceeds_escrow"),
        // Long 10 against Short 5.
        (clear_output(id, Some(50), 10, json!([{"index": 0, "side": "Long", "filled_qty": 10}, {"index": 1, "side": "Short", "filled_qty": 5}])), "unbalanced_fills"),
        // A bid index that does not exist.
        (clear_output(id, Some(50), 1, json!([{"index": 7, "side": "Long", "filled_qty": 1}])), "bad_fill_index"),
        // Another market's result.
        (clear_output(id + 1, Some(50), 0, json!([])), "wrong_market"),
    ];
    for (out, reason) in cases {
        set_mock(&e, Some(out)).await?;
        let r = call(&e.alice, &e, "clear_auction", json!({"market_id": id.to_string()})).await?;
        assert!(has_log(&r, "enclave_rejected") && has_log(&r, reason), "{reason}: {:?}", r.logs());
        assert_eq!(market(&e, id).await?["status"], json!("Auction"));
    }
    // The enclave run failed (OutLayer returns null).
    set_mock(&e, None).await?;
    let r = call(&e.alice, &e, "clear_auction", json!({"market_id": id.to_string()})).await?;
    assert!(has_log(&r, "enclave_no_output"), "{:?}", r.logs());

    // A good result now clears it.
    set_mock(&e, Some(clear_output(id, Some(50), 10, json!([{"index": 0, "side": "Long", "filled_qty": 10}, {"index": 1, "side": "Short", "filled_qty": 10}])))).await?;
    let r = call(&e.alice, &e, "clear_auction", json!({"market_id": id.to_string()})).await?;
    assert!(has_log(&r, "auction_cleared"), "{:?}", r.logs());
    assert_eq!(market(&e, id).await?["status"], json!("Open"));
    Ok(())
}

#[tokio::test]
async fn enclave_callback_rejects_outside_callers() -> anyhow::Result<()> {
    let e = private_with_key().await?;
    // Id 0 was the key request (already used) -> ERR_UNKNOWN_REQUEST; a never-issued id from an
    // outside account -> ERR_NOT_OUTLAYER.
    assert!(fails_with(&call(&e.alice, &e, "on_enclave_result", json!({"pending_id": "0"})).await?, "ERR_UNKNOWN_REQUEST"));
    assert!(fails_with(&call(&e.alice, &e, "on_enclave_result", json!({"pending_id": "999"})).await?, "ERR_NOT_OUTLAYER"));
    Ok(())
}

#[tokio::test]
async fn unanswered_private_auction_can_be_abandoned_and_refunds_everyone() -> anyhow::Result<()> {
    let e = private_with_key().await?;
    let id = create(&e).await?;
    commit_sealed(&e, &e.alice, id, 6 * ONE, "YQ==").await?.into_result()?;
    commit_sealed(&e, &e.bob, id, 5 * ONE, "Yg==").await?.into_result()?;
    assert!(fails_with(&call(&e.bob, &e, "abandon_auction", json!({"market_id": id.to_string()})).await?, "ERR_TOO_EARLY"));
    advance_to(&e.worker, n(&market(&e, id).await?["void_after"])).await?;
    let r = call(&e.bob, &e, "abandon_auction", json!({"market_id": id.to_string()})).await?;
    assert!(r.is_success() && has_log(&r, "auction_abandoned"), "{:?}", r.failures());
    assert_eq!(market(&e, id).await?["status"], json!("Void"));
    assert_eq!(refund_of(&e, &e.alice).await?, 6 * ONE);
    assert_eq!(refund_of(&e, &e.bob).await?, 5 * ONE);
    Ok(())
}

// ---------------- claim tokens (Rhea) ----------------

async fn open_public_market_with_claims(e: &Env, amount: u128) -> anyhow::Result<u64> {
    let id = create(e).await?;
    advance_to(&e.worker, n(&market(e, id).await?["auction_reveal_end"])).await?;
    call(&e.alice, e, "clear_auction", json!({"market_id": id.to_string()})).await?.into_result()?;
    ft_call(e, &e.alice, amount, json!({"action": "mint_pair", "market_id": id.to_string()})).await?.into_result()?;
    Ok(id)
}

async fn deploy_token(e: &Env, name: &str, market: u64, side: &str) -> anyhow::Result<Contract> {
    let root = e.worker.root_account()?;
    let acc = root.create_subaccount(name).initial_balance(NearToken::from_near(10)).transact().await?.into_result()?;
    let token = acc.deploy(&wasm("claim_token")).await?.into_result()?;
    token.call("new").args_json(json!({"flatrate_id": e.flat.id(), "market_id": market.to_string(), "side": side})).transact().await?.into_result()?;
    Ok(token)
}

async fn ft_balance(token: &Contract, who: &str) -> anyhow::Result<u128> {
    let v: String = token.view("ft_balance_of").args_json(json!({"account_id": who})).await?.json()?;
    Ok(v.parse()?)
}

async fn wrap(e: &Env, who: &Account, token: &Contract, market: u64, side: &str, amount: u128) -> anyhow::Result<ExecutionFinalResult> {
    call(who, e, "claim_transfer_call", json!({"receiver_id": token.id(), "market_id": market.to_string(), "side": side,
                                             "amount": amount.to_string(), "msg": ""})).await
}

#[tokio::test]
async fn claims_wrap_into_a_token_move_as_nep141_and_unwrap() -> anyhow::Result<()> {
    let e = setup(false).await?;
    let id = open_public_market_with_claims(&e, 10 * ONE).await?;
    let long = deploy_token(&e, "long", id, "Long").await?;

    let r = wrap(&e, &e.alice, &long, id, "Long", 4 * ONE).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(ft_balance(&long, e.alice.id().as_str()).await?, 4 * ONE);
    assert_eq!(claims(&e, e.alice.id().as_str(), id, "Long").await?, 6 * ONE);
    assert_eq!(claims(&e, long.id().as_str(), id, "Long").await?, 4 * ONE, "the token contract holds the claims");

    // NEP-141 transfer to bob, then bob unwraps into claims of his own.
    e.bob.call(long.id(), "storage_deposit").args_json(json!({"account_id": e.bob.id()})).deposit(NearToken::from_millinear(10)).transact().await?.into_result()?;
    e.alice.call(long.id(), "ft_transfer").args_json(json!({"receiver_id": e.bob.id(), "amount": (3 * ONE).to_string()}))
        .deposit(NearToken::from_yoctonear(1)).transact().await?.into_result()?;
    let r = e.bob.call(long.id(), "unwrap").args_json(json!({"amount": (3 * ONE).to_string()})).deposit(NearToken::from_yoctonear(1)).gas(Gas::from_tgas(100)).transact().await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(ft_balance(&long, e.bob.id().as_str()).await?, 0);
    assert_eq!(claims(&e, e.bob.id().as_str(), id, "Long").await?, 3 * ONE);
    assert_eq!(claims(&e, long.id().as_str(), id, "Long").await?, ONE);
    let supply: String = long.view("ft_total_supply").await?.json()?;
    assert_eq!(supply, ONE.to_string(), "supply equals the claims the token holds");
    Ok(())
}

#[tokio::test]
async fn wrong_side_or_market_is_returned_and_only_flatrate_mints() -> anyhow::Result<()> {
    let e = setup(false).await?;
    let id = open_public_market_with_claims(&e, 10 * ONE).await?;
    let long = deploy_token(&e, "long", id, "Long").await?;
    // Short claims sent to the Long token come back to the sender.
    let r = wrap(&e, &e.alice, &long, id, "Short", 2 * ONE).await?;
    assert!(r.is_success(), "{:?}", r.failures());
    assert_eq!(claims(&e, e.alice.id().as_str(), id, "Short").await?, 10 * ONE);
    assert_eq!(ft_balance(&long, e.alice.id().as_str()).await?, 0);
    // Nobody but Flatrate can mint.
    let r = e.alice.call(long.id(), "on_claim_transfer")
        .args_json(json!({"sender_id": e.alice.id(), "market_id": id.to_string(), "side": "Long", "amount": "1000000", "msg": ""}))
        .transact().await?;
    assert!(fails_with(&r, "ERR_NOT_FLATRATE"));
    Ok(())
}
