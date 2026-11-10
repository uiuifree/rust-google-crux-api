# google-crux-api

[![Crates.io](https://img.shields.io/crates/v/google-crux-api?style=flat-square)](https://crates.io/crates/google-crux-api)
[![Documentation](https://img.shields.io/docsrs/google-crux-api?style=flat-square)](https://docs.rs/google-crux-api)
[![CI](https://img.shields.io/github/actions/workflow/status/uiuifree/rust-google-crux-api/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/uiuifree/rust-google-crux-api/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/google-crux-api?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/LICENSE)

Async Rust client for the [Chrome UX Report (CrUX) API](https://developer.chrome.com/docs/crux/api)
and the [CrUX History API](https://developer.chrome.com/docs/crux/history-api): Core Web Vitals field
data for an origin or page URL, as the latest 28-day aggregate or as a weekly history of up to 40
periods.

## Setup

Enable the **Chrome UX Report API** in a Google Cloud project and create an API key. The API accepts
only API keys; OAuth access tokens are rejected with `400 INVALID_ARGUMENT`.

```toml
[dependencies]
google-crux-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Usage

```rust,no_run
use google_crux_api::{CruxApi, QueryRecordRequest};

#[tokio::main]
async fn main() -> Result<(), google_crux_api::GoogleApiError> {
    let api = CruxApi::with_api_key("YOUR_API_KEY");

    let response = api.query_record(&QueryRecordRequest::origin("https://example.com")).await?;
    let vitals = response.record.core_web_vitals();

    println!("passes Core Web Vitals: {:?}", vitals.passes());
    if let Some(lcp) = vitals.lcp {
        println!("LCP p75 {} ms, rated {:?}, good {:?}", lcp.p75, lcp.rating, lcp.good);
    }
    Ok(())
}
```

`passes()` needs LCP and CLS and returns `None` without them. INP is skipped when it has no data.

Requests time out after 30 seconds. Use `with_client` to pass your own `reqwest::Client`, in which
case its timeout applies instead.

### Filters and metrics

Use `QueryRecordRequest::url` for a single page. Metric names are in the `metric` module, and all
metrics are returned when none is set. Other names can be passed as strings.

```rust
use google_crux_api::{metric, EffectiveConnectionType, FormFactor, QueryRecordRequest};

let request = QueryRecordRequest::url("https://example.com/pricing")
    .form_factor(FormFactor::Phone)
    .effective_connection_type(EffectiveConnectionType::FourG)
    .metrics(metric::CORE_WEB_VITALS);
```

### History

Each period covers 28 days and ends one week after the previous one, oldest first. The default is
25 periods.

```rust,no_run
use google_crux_api::{metric, CruxApi, QueryHistoryRequest};

# async fn run(api: CruxApi) -> Result<(), google_crux_api::GoogleApiError> {
let request = QueryHistoryRequest::origin("https://example.com")
    .metric(metric::INTERACTION_TO_NEXT_PAINT)
    .collection_period_count(12);
let response = api.query_history_record(&request).await?;

for (period, p75) in response.record.p75_series(metric::INTERACTION_TO_NEXT_PAINT) {
    // p75 is None for periods without data
    println!("{} to {}: {:?}", period.first_date, period.last_date, p75);
}
# Ok(())
# }
```

Histogram densities per period are in `metric(..).histogram_timeseries`.

### Other metrics

```rust
# use google_crux_api::{metric, Record};
# fn show(record: &Record) {
let rtt = record.p75(metric::ROUND_TRIP_TIME);
let load_delay = record.p75(metric::LARGEST_CONTENTFUL_PAINT_IMAGE_RESOURCE_LOAD_DELAY);

if let Some(nav) = record.metric(metric::NAVIGATION_TYPES) {
    println!("bfcache hits: {:?}", nav.fractions.get("back_forward_cache"));
}
# }
```

`query_record_raw` and `query_history_record_raw` return the JSON as is.

### Errors

```rust,no_run
use google_crux_api::{CruxApi, GoogleApiError, QueryRecordRequest};

# async fn run(api: CruxApi) {
match api.query_record(&QueryRecordRequest::url("https://example.com/new-page")).await {
    Ok(response) => println!("{:?}", response.record.core_web_vitals()),
    Err(e) if e.is_not_found() => println!("not enough traffic for CrUX data"),
    Err(e) if e.is_timeout() => eprintln!("timed out"),
    Err(GoogleApiError::HttpStatus(429, _)) => eprintln!("quota exhausted"),
    Err(e) => eprintln!("{e} ({:?})", e.reason()),
}
# }
```

| Status | Meaning |
|--------|---------|
| `404` | no CrUX data for this origin or URL with these filters |
| `400` | invalid request, such as an origin with a path, or an invalid API key |
| `403` | missing API key, or the API is not enabled |
| `429` | quota exhausted |

The quota is 150 queries per minute per Google Cloud project, shared by both endpoints. This crate
does not retry.

## Thresholds

| Metric | Unit | Good | Poor |
|--------|------|------|------|
| LCP | ms | ≤ 2500 | > 4000 |
| INP | ms | ≤ 200 | > 500 |
| CLS | unitless | ≤ 0.1 | > 0.25 |
| FCP | ms | ≤ 1800 | > 3000 |
| TTFB | ms | ≤ 800 | > 1800 |

`metric::thresholds` and `metric::rate` expose these values.

## Development

```bash
cargo test
CRUX_API_KEY=... cargo test --test live -- --ignored
CRUX_API_KEY=... cargo run --example query -- https://example.com
```

See [CONTRIBUTING.md](https://github.com/uiuifree/rust-google-crux-api/blob/main/CONTRIBUTING.md).

## License

MIT
