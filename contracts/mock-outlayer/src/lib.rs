//! MOCK. Sandbox tests only; never deployed to testnet, never referenced by the frontend.
//!
//! Stands in for `outlayer.testnet` `request_execution`. The real contract pauses with
//! yield/resume and later returns the module's stdout JSON as the promise value (or `null`
//! on a failed run). This mock returns a value set by the test straight away, so the caller's
//! callback sees the same shape (near-outlayer `contract/src/execution.rs:893-916`).

use near_sdk::serde_json::Value;
use near_sdk::{env, near, AccountId, PanicOnDefault};

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MockMode {
    /// Return `mock_next_output` as the promise value.
    Respond,
    /// Return `null` (OutLayer's value for a failed run or a timeout).
    Null,
    /// Panic, like `request_execution` refusing the call at the door.
    Panic,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct MockOutlayer {
    mode: MockMode,
    next_output: String,
    calls: u64,
    last_args: String,
}

#[near]
impl MockOutlayer {
    #[init]
    pub fn new() -> Self {
        Self { mode: MockMode::Null, next_output: "null".into(), calls: 0, last_args: String::new() }
    }

    /// Test control: what the next `request_execution` returns.
    pub fn mock_set_next(&mut self, mode: MockMode, output: Option<String>) {
        self.mode = mode;
        self.next_output = output.unwrap_or_else(|| "null".into());
    }

    #[payable]
    #[allow(clippy::too_many_arguments)]
    pub fn request_execution(
        &mut self,
        source: Value,
        resource_limits: Option<Value>,
        input_data: Option<String>,
        secrets_ref: Option<Value>,
        response_format: Option<String>,
        payer_account_id: Option<AccountId>,
        params: Option<Value>,
    ) {
        if self.mode == MockMode::Panic {
            env::panic_str("mock: request refused");
        }
        assert!(!env::attached_deposit().is_zero(), "mock: no deposit");
        assert!(resource_limits.is_some(), "mock: resource_limits missing (compile-only)");
        self.calls += 1;
        self.last_args = near_sdk::serde_json::json!({
            "source": source, "resource_limits": resource_limits, "input_data": input_data,
            "secrets_ref": secrets_ref, "response_format": response_format,
            "payer_account_id": payer_account_id, "params": params,
        })
        .to_string();
        let out = match self.mode {
            MockMode::Respond => self.next_output.clone(),
            _ => "null".to_string(),
        };
        env::value_return(out.as_bytes());
    }

    pub fn mock_calls(&self) -> u64 {
        self.calls
    }

    pub fn mock_last_args(&self) -> String {
        self.last_args.clone()
    }
}
