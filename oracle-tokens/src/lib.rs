//! Flatrate token-price oracle (extra 2): parsing and aggregation, free of I/O so it runs in
//! native tests against `fixtures/tokens/` and inside the OutLayer WASI component unchanged.
//!
//! Index `TOKEN_PRICE`: the output (completion) price of Llama 3.3 70B Instruct, in micro-USD per
//! 1M tokens, per OpenRouter provider; the oracle returns the median of the providers listed in
//! the input. One HTTP request: OpenRouter's public endpoints list for the model (no key).
//! Selection rule in FACTS.md section 14.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INDEX_TOKENS: &str = "TOKEN_PRICE";
pub const MODEL: &str = "meta-llama/llama-3.3-70b-instruct";
pub const ENDPOINTS_URL: &str = "https://openrouter.ai/api/v1/models/meta-llama/llama-3.3-70b-instruct/endpoints";

#[derive(Deserialize, Debug)]
pub struct Input {
    pub index: String,
    /// OpenRouter provider names (case-insensitive), each one counts as a source.
    pub sources: Vec<String>,
    /// FIXTURE mode for tests only: a recorded endpoints response. When present, no HTTP request
    /// is made. The Flatrate contract never sends this field.
    #[serde(default)]
    pub fixture_response: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SourceResult {
    pub name: String,
    pub price_micros: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Output {
    pub index: String,
    pub price_micros: Option<u64>,
    pub sources: Vec<SourceResult>,
    pub fetched_at: u64,
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub fixture: bool,
}

/// USD per token (a decimal string) -> micro-USD per 1M tokens.
pub fn per_token_to_micros_per_million(s: &str) -> Result<u64, String> {
    let usd: f64 = s.trim().parse().map_err(|_| format!("bad price {s:?}"))?;
    if !usd.is_finite() || usd <= 0.0 || usd > 0.01 {
        return Err(format!("price out of range: {usd}"));
    }
    Ok((usd * 1e12).round() as u64)
}

/// One price per requested provider from the endpoints response: the lowest completion price
/// among that provider's endpoints for the model (a provider can list several regions).
pub fn parse_endpoints(body: &[u8], providers: &[String]) -> Result<Vec<SourceResult>, String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("bad json: {e}"))?;
    if v["data"]["id"] != MODEL {
        return Err(format!("unexpected model {}", v["data"]["id"]));
    }
    let eps = v["data"]["endpoints"].as_array().ok_or("no endpoints")?;
    Ok(providers
        .iter()
        .map(|want| {
            let prices: Vec<Result<u64, String>> = eps
                .iter()
                .filter(|e| e["provider_name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(want)))
                .map(|e| e["pricing"]["completion"].as_str().ok_or("no completion price".to_string()).and_then(per_token_to_micros_per_million))
                .collect();
            let ok: Vec<u64> = prices.iter().filter_map(|p| p.as_ref().ok().copied()).collect();
            match ok.iter().min() {
                Some(p) => SourceResult { name: want.clone(), price_micros: Some(*p), error: None },
                None => SourceResult {
                    name: want.clone(),
                    price_micros: None,
                    error: Some(prices.into_iter().find_map(|p| p.err()).unwrap_or_else(|| "provider not listed".into())),
                },
            }
        })
        .collect())
}

pub fn aggregate(results: Vec<SourceResult>, fetched_at: u64, request_id: Option<String>, fixture: bool) -> Output {
    let ok: Vec<u64> = results.iter().filter_map(|r| r.price_micros).collect();
    Output { index: INDEX_TOKENS.into(), price_micros: flatrate_shared::median(&ok), sources: results, fetched_at, request_id, fixture }
}

/// Runs the oracle with the given fetcher (HTTP GET in the component).
pub fn run(input: &Input, fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>, fetched_at: u64, request_id: Option<String>) -> Output {
    let fail = |e: String| input.sources.iter().map(|s| SourceResult { name: s.clone(), price_micros: None, error: Some(e.clone()) }).collect();
    let fixture = input.fixture_response.is_some();
    if input.index != INDEX_TOKENS {
        return aggregate(fail("unsupported index".into()), fetched_at, request_id, fixture);
    }
    let body = match &input.fixture_response {
        Some(b) => Ok(b.as_bytes().to_vec()),
        None => fetch(ENDPOINTS_URL),
    };
    let results = match body.and_then(|b| parse_endpoints(&b, &input.sources)) {
        Ok(r) => r,
        Err(e) => fail(e),
    };
    aggregate(results, fetched_at, request_id, fixture)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        let p = format!("{}/../fixtures/tokens/openrouter-llama-3.3-70b-endpoints.json", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
    }
    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn converts_per_token_to_per_million() {
        assert_eq!(per_token_to_micros_per_million("0.00000032"), Ok(320_000));
        assert_eq!(per_token_to_micros_per_million("0.0000012"), Ok(1_200_000));
        assert!(per_token_to_micros_per_million("0").is_err());
        assert!(per_token_to_micros_per_million("abc").is_err());
    }

    #[test]
    fn parses_recorded_endpoints_and_takes_the_median() {
        let input = Input {
            index: INDEX_TOKENS.into(),
            sources: names(&["deepinfra", "akashml", "parasail", "groq", "sambanova", "coreweave", "google"]),
            fixture_response: Some(fixture()),
        };
        let out = run(&input, &mut |_| panic!("no HTTP in fixture mode"), 0, None);
        let p: Vec<Option<u64>> = out.sources.iter().map(|s| s.price_micros).collect();
        assert_eq!(p, vec![Some(320_000), Some(520_000), Some(500_000), Some(790_000), Some(1_200_000), Some(710_000), Some(720_000)]);
        assert_eq!(out.price_micros, Some(710_000)); // median of 7
        assert!(out.fixture);
    }

    #[test]
    fn unknown_provider_and_other_index_are_errors() {
        let input = Input { index: INDEX_TOKENS.into(), sources: names(&["groq", "nobody"]), fixture_response: Some(fixture()) };
        let out = run(&input, &mut |_| unreachable!(), 0, None);
        assert_eq!(out.sources[1].error.as_deref(), Some("provider not listed"));
        let input = Input { index: "H100_HOURLY".into(), sources: names(&["groq"]), fixture_response: Some(fixture()) };
        assert_eq!(run(&input, &mut |_| unreachable!(), 0, None).price_micros, None);
    }
}
