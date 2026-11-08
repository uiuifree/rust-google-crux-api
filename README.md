# google-crux-api — Rust client for the Chrome UX Report API

[![Crates.io](https://img.shields.io/crates/v/google-crux-api?style=flat-square)](https://crates.io/crates/google-crux-api)
[![Documentation](https://img.shields.io/docsrs/google-crux-api?style=flat-square)](https://docs.rs/google-crux-api)
[![CI](https://img.shields.io/github/actions/workflow/status/uiuifree/rust-google-crux-api/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/uiuifree/rust-google-crux-api/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/google-crux-api?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/LICENSE)

Async Rust client for the
[Chrome UX Report (CrUX) API](https://developer.chrome.com/docs/crux/api), the source of the
Core Web Vitals field data that Google uses in Search. It covers both endpoints:

| Endpoint | Method | Returns |
| --- | --- | --- |
| `records:queryRecord` | `CruxApi::query_record` | the latest 28-day aggregate |
| `records:queryHistoryRecord` | `CruxApi::query_history_record` | up to 40 weekly 28-day aggregates |

Queries can target an origin or a single URL, filtered by form factor (phone / desktop / tablet)
and, for the current record, by connection type. Every metric is returned: LCP, INP, CLS, FCP,
TTFB, round trip time, the LCP image subparts, and the navigation type, form factor and LCP
resource type fractions.

Compared with PageSpeed Insights, a CrUX call returns in well under a second because it does not
run Lighthouse, and it can return the history.

## Installation

```toml
[dependencies]
google-crux-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick start

```rust,no_run
use google_crux_api::{metric, CruxApi, FormFactor, QueryHistoryRequest, QueryRecordRequest};

#[tokio::main]
async fn main() -> Result<(), google_crux_api::GoogleApiError> {
    let api = CruxApi::with_api_key("YOUR_API_KEY");

    // Current Core Web Vitals of an origin on phones
    let request = QueryRecordRequest::origin("https://example.com").form_factor(FormFactor::Phone);
    match api.query_record(&request).await {
        Ok(response) => {
            let vitals = response.record.core_web_vitals();
            println!("passes: {:?}", vitals.passes());
            if let Some(lcp) = vitals.lcp {
                println!("LCP p75 {} ms ({:?}), good {:?}", lcp.p75, lcp.rating, lcp.good);
            }
        }
        // 404 means CrUX has no data for this origin or URL
        Err(e) if e.is_not_found() => println!("no data"),
        Err(e) => return Err(e),
    }

    // Weekly history of INP for one page
    let request = QueryHistoryRequest::url("https://example.com/pricing")
        .metric(metric::INTERACTION_TO_NEXT_PAINT)
        .collection_period_count(12);
    let history = api.query_history_record(&request).await?;
    for (period, p75) in history.record.p75_series(metric::INTERACTION_TO_NEXT_PAINT) {
        println!("{}..{}: {:?}", period.first_date, period.last_date, p75);
    }
    Ok(())
}
```

## Authentication

The API needs credentials. Enable the Chrome UX Report API in a Google Cloud project and create
an API key under *APIs & Services → Credentials*, then use `CruxApi::with_api_key`.
`CruxApi::with_access_token` sends an OAuth 2.0 access token instead. The default quota is
150 queries per minute per project and is free.

## Values

- Numbers are `Option<f64>`. The API sends CLS values as strings (`"0.05"`) and marks history
  periods without data as `"NaN"` or `null`; these become numbers and `None`.
- `metric::rate` and `MetricSummary::rating` use the published thresholds: LCP 2.5 s / 4 s,
  INP 200 ms / 500 ms, CLS 0.1 / 0.25, FCP 1.8 s / 3 s, TTFB 0.8 s / 1.8 s.
- `CoreWebVitals::passes` is `true` when every Core Web Vital with data is good. INP is skipped
  when it has no data.
- History periods are 28 days long and end one week apart, oldest first.

## Errors

All methods return `Result<_, GoogleApiError>`:

- `HttpStatus(status, body)`: `404` (`is_not_found()`) when there is no data, which is normal for
  pages with little traffic; `400` for an invalid request; `403` for a missing or invalid key;
  `429` when the quota is used up.
- `InvalidArgument`: rejected before sending (no `http://` / `https://`, or
  `collection_period_count` outside 1 to 40).
- `Connection`, `JsonParse`.

## Examples

```bash
CRUX_API_KEY=... cargo run --example query -- https://example.com
```

## Testing

```bash
cargo test
```

Tests use a mock server, so they need no API key.

## License

MIT

## 日本語での概要

Chrome UX Report (CrUX) API の非同期 Rust クライアントです。実際の Chrome ユーザーの Core Web Vitals（LCP / INP / CLS など）を、オリジン単位または URL 単位で取得できます。直近 28 日の値（`query_record`）と、週ごとの推移を最大 40 期間分（`query_history_record`）取れます。Google Cloud で Chrome UX Report API を有効にして API キーを発行して使ってください。
