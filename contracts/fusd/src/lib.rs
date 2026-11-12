//! fUSD: test stablecoin for Flatrate. NEP-141 + NEP-145 + NEP-148, 6 decimals.
//! Testnet only. Anyone can call `faucet()` for up to 1,000 fUSD per call.

use near_contract_standards::fungible_token::events::FtMint;
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::{
    FungibleToken, FungibleTokenCore, FungibleTokenResolver,
};
use near_contract_standards::storage_management::{
    StorageBalance, StorageBalanceBounds, StorageManagement,
};
use near_sdk::json_types::U128;
use near_sdk::{env, near, require, AccountId, BorshStorageKey, NearToken, PanicOnDefault, PromiseOrValue};

/// 1 fUSD in base units (6 decimals).
pub const ONE_FUSD: u128 = 1_000_000;
/// Most fUSD one `faucet()` call mints.
pub const FAUCET_MAX: u128 = 1_000 * ONE_FUSD;

#[derive(BorshStorageKey)]
#[near]
enum StorageKey {
    Accounts,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
}

#[near]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Self { token: FungibleToken::new(StorageKey::Accounts) }
    }

    /// Mint up to 1,000 fUSD (default 1,000) to the caller. Registers the caller for storage
    /// if needed, paid by this contract (testnet only).
    pub fn faucet(&mut self, amount: Option<U128>) -> U128 {
        let amount = amount.map(|a| a.0).unwrap_or(FAUCET_MAX);
        require!(amount > 0, "ERR_ZERO_AMOUNT");
        require!(amount <= FAUCET_MAX, "ERR_FAUCET_MAX_1000_FUSD");
        let account_id = env::predecessor_account_id();
        if !self.token.accounts.contains_key(&account_id) {
            self.token.internal_register_account(&account_id);
        }
        self.token.internal_deposit(&account_id, amount);
        FtMint { owner_id: &account_id, amount: U128(amount), memo: Some("faucet") }.emit();
        U128(amount)
    }
}

#[near]
impl FungibleTokenCore for Contract {
    #[payable]
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>) {
        self.token.ft_transfer(receiver_id, amount, memo)
    }

    #[payable]
    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
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
    fn ft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        amount: U128,
    ) -> U128 {
        self.token.ft_resolve_transfer(sender_id, receiver_id, amount)
    }
}

#[near]
impl StorageManagement for Contract {
    #[payable]
    fn storage_deposit(
        &mut self,
        account_id: Option<AccountId>,
        registration_only: Option<bool>,
    ) -> StorageBalance {
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
        FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: "Flatrate test USD (testnet only)".to_string(),
            symbol: "fUSD".to_string(),
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

    fn ctx(who: usize) {
        testing_env!(VMContextBuilder::new().predecessor_account_id(accounts(who)).build());
    }

    #[test]
    fn faucet_default_and_cap() {
        ctx(1);
        let mut c = Contract::new();
        assert_eq!(c.faucet(None).0, FAUCET_MAX);
        assert_eq!(c.faucet(Some(U128(5 * ONE_FUSD))).0, 5 * ONE_FUSD);
        assert_eq!(c.ft_balance_of(accounts(1)).0, FAUCET_MAX + 5 * ONE_FUSD);
        assert_eq!(c.ft_total_supply().0, FAUCET_MAX + 5 * ONE_FUSD);
    }

    #[test]
    #[should_panic(expected = "ERR_FAUCET_MAX_1000_FUSD")]
    fn faucet_over_cap_fails() {
        ctx(1);
        let mut c = Contract::new();
        c.faucet(Some(U128(FAUCET_MAX + 1)));
    }

    #[test]
    fn metadata_has_6_decimals() {
        ctx(1);
        let c = Contract::new();
        assert_eq!(c.ft_metadata().decimals, 6);
    }
}
