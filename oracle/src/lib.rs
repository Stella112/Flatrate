//! Flatrate oracle: parsing and aggregation, kept free of I/O so it runs in native tests
//! against `fixtures/prices/` and inside the OutLayer WASI component unchanged.
//!
//! Selection rules (H100 SXM 80GB, on-demand, USD per GPU-hour) are recorded in FACTS.md
//! section 4. Prices are converted to micro-USD per GPU-hour.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INDEX_H100: &str = "H100_HOURLY";
pub const ALL_SOURCES: [&str; 5] = ["akash", "vast", "runpod", "shadeform", "verda"];

/// How to fetch one source.
pub struct SourceRequest {
    pub method: &'static str,
    pub url: &'static str,
    pub body: Option<&'static str>,
}

pub fn request_for(source: &str) -> Option<SourceRequest> {
    Some(match source {
        "akash" => SourceRequest {
            method: "GET",
            url: "https://console-api.akash.network/v1/gpu-prices",
            body: None,
        },
        "vast" => SourceRequest {
            method: "GET",
            url: "https://console.vast.ai/api/v0/bundles/?q=%7B%22gpu_name%22%3A%7B%22eq%22%3A%22H100%20SXM%22%7D%2C%22rentable%22%3A%7B%22eq%22%3Atrue%7D%2C%22type%22%3A%22on-demand%22%2C%22limit%22%3A50%7D",
            body: None,
        },
        "runpod" => SourceRequest {
            method: "POST",
            url: "https://api.runpod.io/graphql",
            body: Some(r#"{"query":"query { gpuTypes { id displayName memoryInGb securePrice communityPrice } }"}"#),
        },
        "shadeform" => SourceRequest {
            method: "GET",
            url: "https://api.shadeform.ai/v1/instances/types",
            body: None,
        },
        "verda" => SourceRequest {
            method: "GET",
            url: "https://api.verda.com/v1/instance-types",
            body: None,
        },
        _ => return None,
    })
}

fn usd_to_micros(usd: f64) -> Result<u64, String> {
    if !usd.is_finite() || usd <= 0.0 || usd > 1_000.0 {
        return Err(format!("price out of range: {usd}"));
    }
    Ok((usd * 1_000_000.0).round() as u64)
}

fn json(body: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(body).map_err(|e| format!("bad json: {e}"))
}

/// akash: `models[]` with `model == "h100"` and `interface == "SXM5"` -> `price.med`.
pub fn parse_akash(body: &[u8]) -> Result<u64, String> {
    let v = json(body)?;
    let m = v["models"]
        .as_array()
        .ok_or("no models")?
        .iter()
        .find(|m| m["model"] == "h100" && m["interface"] == "SXM5")
        .ok_or("no h100 SXM5 model")?;
    usd_to_micros(m["price"]["med"].as_f64().ok_or("no price.med")?)
}

/// vast: `offers[]` with `gpu_name == "H100 SXM"` and `rentable` -> min of `dph_total / num_gpus`.
pub fn parse_vast(body: &[u8]) -> Result<u64, String> {
    let v = json(body)?;
    let best = v["offers"]
        .as_array()
        .ok_or("no offers")?
        .iter()
        .filter(|o| o["gpu_name"] == "H100 SXM" && o["rentable"] == true)
        .filter_map(|o| {
            let dph = o["dph_total"].as_f64()?;
            let n = o["num_gpus"].as_f64()?;
            (n >= 1.0).then_some(dph / n)
        })
        .fold(f64::INFINITY, f64::min);
    if !best.is_finite() {
        return Err("no rentable H100 SXM offer".into());
    }
    usd_to_micros(best)
}

/// runpod: `data.gpuTypes[]` with `id == "NVIDIA H100 80GB HBM3"` -> `securePrice`.
pub fn parse_runpod(body: &[u8]) -> Result<u64, String> {
    let v = json(body)?;
    let g = v["data"]["gpuTypes"]
        .as_array()
        .ok_or("no gpuTypes")?
        .iter()
        .find(|g| g["id"] == "NVIDIA H100 80GB HBM3")
        .ok_or("no NVIDIA H100 80GB HBM3")?;
    usd_to_micros(g["securePrice"].as_f64().ok_or("no securePrice")?)
}

/// shadeform: `instance_types[]` with `gpu_type == "H100"`, `interconnect == "sxm5"`,
/// `num_gpus == 1` -> median of `hourly_price` (integer cents).
pub fn parse_shadeform(body: &[u8]) -> Result<u64, String> {
    let v = json(body)?;
    let cents: Vec<u64> = v["instance_types"]
        .as_array()
        .ok_or("no instance_types")?
        .iter()
        .filter(|i| {
            let c = &i["configuration"];
            c["gpu_type"] == "H100" && c["interconnect"] == "sxm5" && c["num_gpus"] == 1
        })
        .filter_map(|i| i["hourly_price"].as_u64())
        .collect();
    let med = flatrate_shared::median(&cents).ok_or("no 1x H100 sxm5 instance")?;
    usd_to_micros(med as f64 / 100.0)
}

/// verda: element with `instance_type == "1H100.80S.30V"` -> `price_per_hour` (a string).
pub fn parse_verda(body: &[u8]) -> Result<u64, String> {
    let v = json(body)?;
    let i = v
        .as_array()
        .ok_or("not an array")?
        .iter()
        .find(|i| i["instance_type"] == "1H100.80S.30V")
        .ok_or("no 1H100.80S.30V")?;
    let p = &i["price_per_hour"];
    let usd = match p {
        Value::String(s) => s.parse::<f64>().map_err(|e| format!("bad price_per_hour: {e}"))?,
        Value::Number(n) => n.as_f64().ok_or("bad price_per_hour")?,
        _ => return Err("no price_per_hour".into()),
    };
    usd_to_micros(usd)
}

pub fn parse(source: &str, body: &[u8]) -> Result<u64, String> {
    match source {
        "akash" => parse_akash(body),
        "vast" => parse_vast(body),
        "runpod" => parse_runpod(body),
        "shadeform" => parse_shadeform(body),
        "verda" => parse_verda(body),
        other => Err(format!("unknown source {other}")),
    }
}

#[derive(Deserialize, Debug)]
pub struct Input {
    pub index: String,
    pub sources: Vec<String>,
    /// FIXTURE mode for tests only: recorded response bodies by source name. When present,
    /// no HTTP request is made. The Flatrate contract never sends this field.
    #[serde(default)]
    pub fixture_responses: Option<std::collections::BTreeMap<String, String>>,
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
    /// True only in fixture mode.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub fixture: bool,
}

/// Builds the output from per-source results. `price_micros` is the median of the sources that
/// returned a price, with the same function the contract uses to check it.
pub fn aggregate(index: &str, results: Vec<SourceResult>, fetched_at: u64, request_id: Option<String>, fixture: bool) -> Output {
    let ok: Vec<u64> = results.iter().filter_map(|r| r.price_micros).collect();
    Output {
        index: index.to_string(),
        price_micros: flatrate_shared::median(&ok),
        sources: results,
        fetched_at,
        request_id,
        fixture,
    }
}

pub fn result_from(name: &str, fetched: Result<Vec<u8>, String>) -> SourceResult {
    match fetched.and_then(|body| parse(name, &body)) {
        Ok(p) => SourceResult { name: name.to_string(), price_micros: Some(p), error: None },
        Err(e) => SourceResult { name: name.to_string(), price_micros: None, error: Some(truncate(&e, 200)) },
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Runs the oracle for one input with the given fetcher (HTTP in the component, a map in tests).
pub fn run(input: &Input, fetch: &mut dyn FnMut(&SourceRequest) -> Result<Vec<u8>, String>, fetched_at: u64, request_id: Option<String>) -> Output {
    if input.index != INDEX_H100 {
        let results = input
            .sources
            .iter()
            .map(|s| SourceResult { name: s.clone(), price_micros: None, error: Some("unsupported index".into()) })
            .collect();
        return aggregate(&input.index, results, fetched_at, request_id, input.fixture_responses.is_some());
    }
    let results = input
        .sources
        .iter()
        .map(|name| {
            let fetched = if let Some(fx) = &input.fixture_responses {
                fx.get(name).map(|b| b.as_bytes().to_vec()).ok_or_else(|| "no fixture".to_string())
            } else {
                match request_for(name) {
                    Some(req) => fetch(&req),
                    None => Err("unknown source".to_string()),
                }
            };
            result_from(name, fetched)
        })
        .collect();
    aggregate(&input.index, results, fetched_at, request_id, input.fixture_responses.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let p = format!("{}/../fixtures/prices/{}.json", env!("CARGO_MANIFEST_DIR"), name);
        std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
    }

    #[test]
    fn parses_each_recorded_response() {
        assert_eq!(parse_akash(&fixture("akash")), Ok(2_560_000));
        // offers: 4.4436/1, 4.5234/1, 3.9789/1, 7.9530/2 -> min is the 2-GPU offer per GPU
        let vast = parse_vast(&fixture("vast")).unwrap();
        assert!((3_970_000..3_980_000).contains(&vast), "{vast}");
        assert_eq!(parse_runpod(&fixture("runpod")), Ok(3_990_000));
        assert_eq!(parse_shadeform(&fixture("shadeform")), Ok(3_980_000));
        assert_eq!(parse_verda(&fixture("verda")), Ok(5_087_000));
    }

    #[test]
    fn fixture_run_gives_median_of_five() {
        let fx = ALL_SOURCES
            .iter()
            .map(|s| (s.to_string(), String::from_utf8(fixture(s)).unwrap()))
            .collect();
        let input = Input {
            index: INDEX_H100.into(),
            sources: ALL_SOURCES.iter().map(|s| s.to_string()).collect(),
            fixture_responses: Some(fx),
        };
        let out = run(&input, &mut |_| panic!("no HTTP in fixture mode"), 0, None);
        assert!(out.fixture);
        assert!(out.sources.iter().all(|s| s.price_micros.is_some()), "{:?}", out.sources);
        assert_eq!(out.price_micros, Some(3_980_000));
    }

    #[test]
    fn failed_sources_are_reported_not_fatal() {
        let input = Input { index: INDEX_H100.into(), sources: vec!["akash".into(), "nope".into(), "vast".into()], fixture_responses: None };
        let out = run(&input, &mut |req| if req.url.contains("akash") { Ok(fixture("akash")) } else { Err("http 503".into()) }, 1, Some("42".into()));
        assert_eq!(out.sources[0].price_micros, Some(2_560_000));
        assert_eq!(out.sources[1].error.as_deref(), Some("unknown source"));
        assert_eq!(out.sources[2].error.as_deref(), Some("http 503"));
        assert_eq!(out.price_micros, Some(2_560_000));
        assert_eq!(out.request_id.as_deref(), Some("42"));
    }

    #[test]
    fn rejects_garbage_and_out_of_range() {
        assert!(parse_akash(b"not json").is_err());
        assert!(parse_verda(br#"[{"instance_type":"1H100.80S.30V","price_per_hour":"0"}]"#).is_err());
        assert!(parse_runpod(br#"{"data":{"gpuTypes":[]}}"#).is_err());
    }

    #[test]
    fn output_shape_matches_contract() {
        let out = aggregate(INDEX_H100, vec![SourceResult { name: "akash".into(), price_micros: Some(1), error: None }], 5, None, false);
        let v: Value = serde_json::to_value(&out).unwrap();
        assert_eq!(v["index"], "H100_HOURLY");
        assert_eq!(v["price_micros"], 1);
        assert_eq!(v["sources"][0]["name"], "akash");
        assert!(v.get("fixture").is_none(), "flag only appears in fixture mode");
    }
}
