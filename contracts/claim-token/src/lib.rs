//! Flatrate claims as a NEP-141 token (extra: Rhea pool).
//!
//! One deployment per (Flatrate contract, market, side). One token unit = one claim unit (6
//! decimals, like fUSD), so 1.000000 token = one whole Long (or Short) claim of that market.
//!
//! Wrap: the holder calls Flatrate `claim_transfer_call(receiver_id = this, market_id, side,
//! amount, msg)`; Flatrate moves the claims to this contract and calls `on_claim_transfer`, which
//! mints the same amount of tokens to the holder. Unwrap: `unwrap(amount)` burns the caller's
//! tokens and sends the claims back with Flatrate `claim_transfer`. The claims behind every token
//! sit in this contract's Flatrate balance, so `ft_total_supply` equals that balance.
//! As a NEP-141 token it can be deposited into a Rhea (Ref) pool and swapped.

use near_contract_standards::fungible_token::events::{FtBurn, FtMint};
use near_contract_standards::fungible_token::metadata::{FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC};
use near_contract_standards::fungible_token::{FungibleToken, FungibleTokenCore, FungibleTokenResolver};
use near_contract_standards::storage_management::{StorageBalance, StorageBalanceBounds, StorageManagement};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde_json::json;
use near_sdk::{env, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseError, PromiseOrValue};

const GAS_CLAIM_TRANSFER: Gas = Gas::from_tgas(15);
const GAS_ON_UNWRAP: Gas = Gas::from_tgas(10);

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Accounts,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    flatrate_id: AccountId,
    market_id: u64,
    /// "Long" or "Short", as Flatrate writes `Side` in JSON.
    side: String,
}

#[near]
impl Contract {
    #[init]
    pub fn new(flatrate_id: AccountId, market_id: U64, side: String) -> Self {
        require!(side == "Long" || side == "Short", "ERR_BAD_SIDE");
        Self { token: FungibleToken::new(StorageKey::Accounts), flatrate_id, market_id: market_id.0, side }
    }

    /// Called by Flatrate after `claim_transfer_call` moved claims here. Mints the same amount to
    /// `sender_id` and returns 0 (nothing unused). Claims of another market or side are returned.
    pub fn on_claim_transfer(&mut self, sender_id: AccountId, market_id: U64, side: String, amount: U128, msg: String) -> U128 {
        let _ = msg;
        require!(env::predecessor_account_id() == self.flatrate_id, "ERR_NOT_FLATRATE");
        if market_id.0 != self.market_id || side != self.side || amount.0 == 0 {
            return amount;
        }
        if !self.token.accounts.contains_key(&sender_id) {
            self.token.internal_register_account(&sender_id);
        }
        self.token.internal_deposit(&sender_id, amount.0);
        FtMint { owner_id: &sender_id, amount, memo: Some("claims wrapped") }.emit();
        U128(0)
    }

    /// Burns `amount` of the caller's tokens and sends the same amount of claims back to the
    /// caller in Flatrate. If that transfer fails the tokens are minted back.
    #[payable]
    pub fn unwrap(&mut self, amount: U128) -> Promise {
        near_sdk::assert_one_yocto();
        let account_id = env::predecessor_account_id();
        require!(amount.0 > 0, "ERR_ZERO_AMOUNT");
        self.token.internal_withdraw(&account_id, amount.0);
        FtBurn { owner_id: &account_id, amount, memo: Some("claims unwrapped") }.emit();
        Promise::new(self.flatrate_id.clone())
            .function_call(
                "claim_transfer".to_string(),
                json!({"receiver_id": account_id, "market_id": U64(self.market_id), "side": self.side, "amount": amount})
                    .to_string()
                    .into_bytes(),
                NearToken::from_yoctonear(0),
                GAS_CLAIM_TRANSFER,
            )
            .then(Self::ext(env::current_account_id()).with_static_gas(GAS_ON_UNWRAP).on_unwrap(account_id, amount))
    }

    #[private]
    pub fn on_unwrap(&mut self, account_id: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>) -> bool {
        if result.is_ok() {
            return true;
        }
        self.token.internal_deposit(&account_id, amount.0);
        FtMint { owner_id: &account_id, amount, memo: Some("unwrap failed, restored") }.emit();
        false
    }

    /// What this token stands for.
    pub fn claim_info(&self) -> near_sdk::serde_json::Value {
        json!({"flatrate_id": self.flatrate_id, "market_id": U64(self.market_id), "side": self.side})
    }
}

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>, msg: String) -> PromiseOrValue<U128> {
        self.token.ft_transfer_call(receiver_id, amount, memo, msg)
    }

    fn ft_total_supply(&self) -> U128 {
        self.token.ft_total_supply()
    }

    fn ft_balance_of(&self, account_id: AccountId) -> U128 {
        self.token.ft_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenResolver for Contract {
    #[private]
    fn ft_resolve_transfer(&mut self, sender_id: AccountId, receiver_id: AccountId, amount: U128) -> U128 {
        self.token.ft_resolve_transfer(sender_id, receiver_id, amount)
    }
}

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>) -> StorageBalance {
        self.token.storage_deposit(account_id, registration_only)
    }

    #[payable]
    fn storage_withdraw(&mut self, amount: Option<NearToken>) -> StorageBalance {
        self.token.storage_withdraw(amount)
    }

    #[payable]
    fn storage_unregister(&mut self, force: Option<bool>) -> bool {
        self.token.storage_unregister(force)
    }

    fn storage_balance_bounds(&self) -> StorageBalanceBounds {
        self.token.storage_balance_bounds()
    }

    fn storage_balance_of(&self, account_id: AccountId) -> Option<StorageBalance> {
        self.token.storage_balance_of(account_id)
    }
}

#[near]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        let short = if self.side == "Long" { "L" } else { "S" };
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: format!("Flatrate {} claim, market {} (testnet)", self.side, self.market_id),
            symbol: format!("FR{}{}", short, self.market_id),
            icon: None,
            reference: None,
            reference_hash: None,
            decimals: 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;

    fn ctx(who: AccountId) {
        testing_env!(VMContextBuilder::new().predecessor_account_id(who).attached_deposit(NearToken::from_yoctonear(1)).build());
    }

    fn flatrate() -> AccountId {
        "flatrate.testnet".parse().unwrap()
    }

    #[test]
    fn wrap_mints_and_only_flatrate_can_mint() {
        ctx(flatrate());
        let mut c = Contract::new(flatrate(), U64(20), "Long".into());
        assert_eq!(c.on_claim_transfer(accounts(1), U64(20), "Long".into(), U128(5_000_000), String::new()).0, 0);
        assert_eq!(c.ft_balance_of(accounts(1)).0, 5_000_000);
        // Another market or side: nothing minted, all returned as unused.
        assert_eq!(c.on_claim_transfer(accounts(1), U64(21), "Long".into(), U128(7), String::new()).0, 7);
        assert_eq!(c.on_claim_transfer(accounts(1), U64(20), "Short".into(), U128(7), String::new()).0, 7);
        assert_eq!(c.ft_total_supply().0, 5_000_000);
        assert_eq!(c.ft_metadata().symbol, "FRL20");
    }

    #[test]
    #[should_panic(expected = "ERR_NOT_FLATRATE")]
    fn mint_from_anyone_else_fails() {
        ctx(flatrate());
        let mut c = Contract::new(flatrate(), U64(20), "Long".into());
        ctx(accounts(2));
        c.on_claim_transfer(accounts(2), U64(20), "Long".into(), U128(1), String::new());
    }

    #[test]
    fn unwrap_burns() {
        ctx(flatrate());
        let mut c = Contract::new(flatrate(), U64(20), "Short".into());
        c.on_claim_transfer(accounts(1), U64(20), "Short".into(), U128(3_000_000), String::new());
        ctx(accounts(1));
        let _ = c.unwrap(U128(1_000_000));
        assert_eq!(c.ft_balance_of(accounts(1)).0, 2_000_000);
        assert_eq!(c.ft_total_supply().0, 2_000_000);
    }
}
