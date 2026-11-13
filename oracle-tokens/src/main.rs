//! OutLayer WASI entry point (wasm32-wasip2). Reads the input JSON on stdin, makes one HTTPS GET
//! to OpenRouter, prints one JSON object on stdout (OutLayer `response_format: "Json"`).

use flatrate_oracle_tokens::{run, Input};
use std::io::{self, Read, Write};

#[cfg(target_os = "wasi")]
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    use std::time::Duration;
    use wasi_http_client::Client;
    let resp = Client::new()
        .get(url)
        .header("Accept", "application/json")
        .header("User-Agent", "flatrate-oracle-tokens/0.1")
        .connect_timeout(Duration::from_secs(10))
        .send()
        .map_err(|e| format!("http: {e}"))?;
    let status = resp.status();
    if !(200..300).contains(&status) {
        return Err(format!("http {status}"));
    }
    resp.body().map_err(|e| format!("body: {e}"))
}

#[cfg(not(target_os = "wasi"))]
fn fetch(_url: &str) -> Result<Vec<u8>, String> {
    Err("HTTP is only available in the wasm32-wasip2 build".into())
}

fn main() {
    let mut raw = String::new();
    let out = match io::stdin().read_to_string(&mut raw).map_err(|e| e.to_string()).and_then(|_| {
        serde_json::from_str::<Input>(&raw).map_err(|e| format!("bad input: {e}"))
    }) {
        Ok(input) => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let request_id = std::env::var("NEAR_REQUEST_ID").ok().filter(|s| !s.is_empty());
            serde_json::to_string(&run(&input, &mut fetch, now, request_id)).unwrap()
        }
        Err(e) => serde_json::json!({"index": null, "price_micros": null, "sources": [], "fetched_at": 0, "error": e}).to_string(),
    };
    print!("{out}");
    let _ = io::stdout().flush();
}
