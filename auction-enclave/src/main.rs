//! Entry point: reads one JSON object on stdin, writes one on stdout.
//!
//! `{"op":"pubkey"}` -> `{"op":"pubkey","x25519_pubkey":"<64 hex>","request_id":...}`
//! `{"op":"clear","market_id":"<id>","bids":[{index,bidder,escrow,ciphertext}]}` -> `ClearOutput`
//! Anything else, or a host refusal -> `{"op":"error","error":"..."}` (the contract records nothing).

use flatrate_auction_enclave::{clear, public_key_hex, secret_from_signature, Input, KEY_MESSAGE};
use serde_json::{json, Value};

// The manifest travels in the wasm as the `outlayer.manifest` custom section; the worker reads it
// to issue the signing key (near-outlayer wasi-examples/CONNECTOR_MANIFEST.md).
#[cfg(target_arch = "wasm32")]
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] = *include_bytes!("../manifest.json");

#[cfg(target_arch = "wasm32")]
mod signing_keys_host {
    wit_bindgen::generate!({
        world: "signing-keys-host",
        path: "wit",
    });
}

/// The X25519 secret, derived from the enclave's signing key `auction`.
#[cfg(target_arch = "wasm32")]
fn secret() -> Result<[u8; 32], String> {
    use signing_keys_host::outlayer::signing_keys::api as signing_keys;
    let sig = signing_keys::sign("auction", None, KEY_MESSAGE)?;
    Ok(secret_from_signature(&sig))
}

/// Native builds have no enclave key; used only to check the binary links.
#[cfg(not(target_arch = "wasm32"))]
fn secret() -> Result<[u8; 32], String> {
    let _ = KEY_MESSAGE;
    let _ = secret_from_signature;
    Err("no signing key outside the enclave".to_string())
}

fn run(raw: &str) -> Value {
    let input: Input = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => return json!({"op": "error", "error": format!("bad input: {e}")}),
    };
    let request_id = std::env::var("NEAR_REQUEST_ID").ok().filter(|s| !s.is_empty());
    let sk = match secret() {
        Ok(s) => s,
        Err(e) => return json!({"op": "error", "error": format!("signing key: {e}")}),
    };
    match input.op.as_str() {
        "pubkey" => json!({"op": "pubkey", "x25519_pubkey": public_key_hex(&sk), "request_id": request_id}),
        "clear" => {
            let Some(market_id) = input.market_id else { return json!({"op": "error", "error": "clear needs market_id"}) };
            let mut v = serde_json::to_value(clear(&sk, &market_id, &input.bids)).unwrap_or(Value::Null);
            v["request_id"] = json!(request_id);
            v
        }
        other => json!({"op": "error", "error": format!("unknown op {other:?}")}),
    }
}

fn main() {
    use std::io::{Read, Write};
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let out = run(&raw);
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(out.to_string().as_bytes());
    let _ = stdout.flush();
}
