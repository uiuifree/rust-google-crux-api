# google-crux-api — Rust client for the Chrome UX Report (CrUX) API

[![Crates.io](https://img.shields.io/crates/v/google-crux-api?style=flat-square)](https://crates.io/crates/google-crux-api)
[![Documentation](https://img.shields.io/docsrs/google-crux-api?style=flat-square)](https://docs.rs/google-crux-api)
[![CI](https://img.shields.io/github/actions/workflow/status/uiuifree/rust-google-crux-api/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/uiuifree/rust-google-crux-api/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/google-crux-api?style=flat-square)](https://github.com/uiuifree/rust-google-crux-api/blob/main/LICENSE)

`google-crux-api` is an async Rust library (crate) for the
[Chrome UX Report (CrUX) API](https://developer.chrome.com/docs/crux/api) and the
[CrUX History API](https://developer.chrome.com/docs/crux/history-api). CrUX is the dataset of
real Chrome user experience that Google uses for the **Core Web Vitals** in Search.

With this crate you can get, for any origin or page URL with enough traffic:

- the **75th percentile** and **good / needs improvement / poor** shares of Largest Contentful
  Paint (LCP), Interaction to Next Paint (INP), Cumulative Layout Shift (CLS), First Contentful
  Paint (FCP) and Time to First Byte (TTFB)
- whether the page **passes the Core Web Vitals assessment**
- a **weekly history** of up to 40 periods to see trends and regressions
- breakdowns by device (phone / desktop / tablet) and connection type

A call returns in well under a second because, unlike PageSpeed Insights, it does not run
Lighthouse. That makes it a good fit for monitoring many URLs, dashboards, CI checks, and AI agents
or CLIs that need Core Web Vitals as data.

## Table of contents

- [Features](#features)
- [What this crate does and does not do](#what-this-crate-does-and-does-not-do)
- [API coverage](#api-coverage)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
  - [Origin or URL](#origin-or-url)
  - [Core Web Vitals summary](#core-web-vitals-summary)
  - [Filters and metrics](#filters-and-metrics)
  - [History](#history)
  - [Other metrics](#other-metrics)
  - [Output as JSON](#output-as-json)
  - [Error handling](#error-handling)
- [Values and thresholds](#values-and-thresholds)
- [Quota and rate limits](#quota-and-rate-limits)
- [Examples](#examples)
- [Testing](#testing)
- [FAQ](#faq)
- [Alternatives](#alternatives)
- [Requirements](#requirements)
- [Contributing](#contributing)
- [License](#license)
- [Links](#links)
- [日本語での概要](#日本語での概要)

## Features

- **Both endpoints**: `records:queryRecord` (latest 28 days) and `records:queryHistoryRecord`
  (weekly history)
- **Every request option**: origin or URL, form factor, effective connection type, metric list,
  number of history periods
- **Every metric**, including round trip time, the LCP image subparts and the fraction metrics
- **Clean numbers**: the API sends CLS as strings and missing history points as `"NaN"`; this
  crate turns them into `Option<f64>`
- **Core Web Vitals helpers**: thresholds, ratings, good / poor shares and a pass / fail check
- **Serde everywhere**: every response type and summary implements `Serialize`
- Small dependency set: `reqwest` (rustls), `serde`, `serde_json`

## What this crate does and does not do

| Does | Does not |
|------|----------|
| Build and send CrUX API requests | Run Lighthouse or measure lab data (use [`google-pagespeed-insights-api`](https://crates.io/crates/google-pagespeed-insights-api)) |
| Parse records and histories into Rust types | Query the CrUX BigQuery dataset |
| Rate values against the published Core Web Vitals thresholds | Retry, queue or throttle requests |
| Tell "no data" (404) apart from other errors | Collect your own real user monitoring data |

## API coverage

| CrUX API endpoint | Method in this crate | Returns |
|-------------------|----------------------|---------|
| `POST /v1/records:queryRecord` | `CruxApi::query_record(&QueryRecordRequest)` | `QueryRecordResponse` |
| `POST /v1/records:queryHistoryRecord` | `CruxApi::query_history_record(&QueryHistoryRequest)` | `QueryHistoryResponse` |
| either | `query_record_raw`, `query_history_record_raw` | `serde_json::Value` |

| Request field | Builder method | Notes |
|---------------|----------------|-------|
| `origin` / `url` | `::origin(..)` / `::url(..)` / `::new(Target)` | one of the two |
| `formFactor` | `.form_factor(FormFactor)` | `Phone`, `Desktop`, `Tablet`, `AllFormFactors` |
| `effectiveConnectionType` | `.effective_connection_type(..)` | `queryRecord` only: `4G`, `3G`, `2G`, `slow-2G`, `offline` |
| `metrics` | `.metric(..)`, `.metrics(..)` | all metrics when empty; names in the `metric` module |
| `collectionPeriodCount` | `.collection_period_count(n)` | history only, 1 to 40, default 25 |

## Prerequisites

The CrUX API needs an API key (or an OAuth 2.0 access token):

1. Open the [Google Cloud console](https://console.cloud.google.com/) and select or create a project.
2. Enable the **Chrome UX Report API** under *APIs & Services → Library*.
3. Create an API key under *APIs & Services → Credentials*. Restricting it to the Chrome UX Report
   API is recommended.

The same key can be used for the PageSpeed Insights API if that API is enabled in the project too.

## Installation

```toml
[dependencies]
google-crux-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or run `cargo add google-crux-api`.

## Quick start

```rust,no_run
use google_crux_api::{CruxApi, QueryRecordRequest};

#[tokio::main]
async fn main() -> Result<(), google_crux_api::GoogleApiError> {
    let api = CruxApi::with_api_key("YOUR_API_KEY");

    let response = api.query_record(&QueryRecordRequest::origin("https://example.com")).await?;
    let vitals = response.record.core_web_vitals();

    println!("passes Core Web Vitals: {:?}", vitals.passes());
    if let Some(lcp) = vitals.lcp {
        println!("LCP p75 {} ms, rated {:?}", lcp.p75, lcp.rating);
    }
    Ok(())
}
```

## Usage

### Origin or URL

```rust
use google_crux_api::{QueryRecordRequest, Target};

// The whole site: scheme and host, no path
let origin = QueryRecordRequest::origin("https://example.com");
// One page
let page = QueryRecordRequest::url("https://example.com/pricing");
// Same thing with an explicit target
let page = QueryRecordRequest::new(Target::Url("https://example.com/pricing".into()));
```

Page-level data exists only for URLs with enough traffic. When there is none, the API answers
`404`; check it with `GoogleApiError::is_not_found()` and fall back to the origin.

### Core Web Vitals summary

`Record::core_web_vitals()` returns `CoreWebVitals` with a `MetricSummary` for LCP, INP, CLS, FCP
and TTFB:

```rust
# use google_crux_api::Record;
# fn show(record: &Record) {
let vitals = record.core_web_vitals();
if let Some(inp) = vitals.inp {
    println!("p75 {} ms", inp.p75);                 // 75th percentile
    println!("rating {:?}", inp.rating);            // Some(Good | NeedsImprovement | Poor)
    println!("good {:?}, poor {:?}", inp.good, inp.poor); // share of page loads, 0.0 to 1.0
}
println!("passes: {:?}", vitals.passes());
# }
```

### Filters and metrics

```rust
use google_crux_api::{metric, EffectiveConnectionType, FormFactor, QueryRecordRequest};

let request = QueryRecordRequest::origin("https://example.com")
    .form_factor(FormFactor::Phone)
    .effective_connection_type(EffectiveConnectionType::FourG)
    .metrics(metric::CORE_WEB_VITALS);
```

### History

The history API returns up to 40 periods. Each period covers 28 days and ends one week after the
previous one, oldest first.

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
// Percentile metrics
let rtt = record.p75(metric::ROUND_TRIP_TIME);
let load_delay = record.p75(metric::LARGEST_CONTENTFUL_PAINT_IMAGE_RESOURCE_LOAD_DELAY);

// Fraction metrics
if let Some(nav) = record.metric(metric::NAVIGATION_TYPES) {
    println!("bfcache hits: {:?}", nav.fractions.get("back_forward_cache"));
}
# }
```

### Output as JSON

All response types and summaries implement `Serialize`:

```rust
# use google_crux_api::Record;
# fn show(record: &Record) -> serde_json::Result<()> {
let vitals = record.core_web_vitals();
let json = serde_json::json!({
    "origin": record.key.origin,
    "period_end": record.collection_period.map(|p| p.last_date.to_string()),
    "passes": vitals.passes(),
    "vitals": vitals,
});
println!("{}", serde_json::to_string_pretty(&json)?);
# Ok(())
# }
```

### Error handling

```rust,no_run
use google_crux_api::{CruxApi, GoogleApiError, QueryRecordRequest};

# async fn run(api: CruxApi) {
match api.query_record(&QueryRecordRequest::url("https://example.com/new-page")).await {
    Ok(response) => println!("{:?}", response.record.core_web_vitals()),
    Err(e) if e.is_not_found() => println!("not enough traffic for CrUX data"),
    Err(GoogleApiError::HttpStatus(429, _)) => eprintln!("quota exhausted"),
    Err(GoogleApiError::HttpStatus(status, body)) => eprintln!("{status}: {body}"),
    Err(GoogleApiError::InvalidArgument(e)) => eprintln!("bad input: {e}"),
    Err(e) => eprintln!("{e}"),
}
# }
```

| Status | Meaning |
|--------|---------|
| `404` | no CrUX data for this origin / URL with these filters (`is_not_found()`) |
| `400` | invalid request, for example an origin with a path |
| `403` | missing or invalid API key, or the API is not enabled |
| `429` | quota exhausted |

## Values and thresholds

| Metric | Unit | Good | Poor |
|--------|------|------|------|
| LCP | ms | ≤ 2500 | > 4000 |
| INP | ms | ≤ 200 | > 500 |
| CLS | unitless | ≤ 0.1 | > 0.25 |
| FCP | ms | ≤ 1800 | > 3000 |
| TTFB | ms | ≤ 800 | > 1800 |

`metric::thresholds` and `metric::rate` expose these values. `CoreWebVitals::passes` is `true` when
LCP, INP and CLS are all good; INP is skipped when it has no data, as Google does.

## Quota and rate limits

The API is free. The default quota is **150 queries per minute** per Google Cloud project, shared by
both endpoints. Data is updated daily.

## Examples

```bash
CRUX_API_KEY=... cargo run --example query -- https://example.com
CRUX_API_KEY=... cargo run --example query -- https://example.com/pricing
```

[`examples/query.rs`](https://github.com/uiuifree/rust-google-crux-api/blob/main/examples/query.rs)
prints the Core Web Vitals summary and the LCP history.

## Testing

```bash
cargo test                                       # mock server, no API key needed
CRUX_API_KEY=... cargo test --test live -- --ignored     # real API
```

CI runs the mock tests on every push and the live tests weekly when the `CRUX_API_KEY` secret is
set, so changes in Google's response format are noticed.

## FAQ

**Why does my page return 404?**
CrUX only publishes data for pages and origins with enough Chrome traffic. Query the origin instead,
or use lab data from PageSpeed Insights.

**How is this different from PageSpeed Insights?**
PageSpeed Insights runs Lighthouse (10 to 60 seconds) and adds the current CrUX values. The CrUX API
returns only field data, in well under a second, and has a history endpoint.

**Why are CLS values strings in the raw JSON?**
That is how the API sends them. The typed response converts them to `f64`.

**Is the data the same as in Search Console's Core Web Vitals report?**
Both come from CrUX. Search Console groups similar URLs, while this API reports one origin or URL.

**Which TLS backend is used?**
`reqwest` 0.13 with its default `rustls` backend. No OpenSSL is required.

## Alternatives

- [`google-chromeuxreport1`](https://crates.io/crates/google-chromeuxreport1): generated bindings
  from the `google-apis-rs` project with a generic client.
- The CrUX dataset on BigQuery, for monthly origin-level data across many sites.

## Requirements

- Rust 1.88 or later
- A Tokio runtime

## Contributing

Issues and pull requests are welcome. See
[CONTRIBUTING.md](https://github.com/uiuifree/rust-google-crux-api/blob/main/CONTRIBUTING.md).

## License

MIT. See [LICENSE](https://github.com/uiuifree/rust-google-crux-api/blob/main/LICENSE).

## Links

- [CrUX API documentation](https://developer.chrome.com/docs/crux/api)
- [CrUX History API documentation](https://developer.chrome.com/docs/crux/history-api)
- [API reference](https://developer.chrome.com/docs/crux/reference/rest)
- [Core Web Vitals](https://web.dev/articles/vitals)

## 日本語での概要

Chrome UX Report (CrUX) API の非同期 Rust クライアントです。CrUX は Google 検索の Core Web Vitals 評価に使われている、実際の Chrome ユーザーの体験データです。

- オリジン単位または URL 単位で、LCP / INP / CLS / FCP / TTFB の 75 パーセンタイルと、良好・要改善・不良の割合を取得できます
- `core_web_vitals()` で各指標の判定をまとめて取り出せ、`passes()` で Core Web Vitals に合格しているかを判定できます
- `query_history_record` で、週ごとの推移を最大 40 期間分取得できます
- デバイス（スマートフォン / PC / タブレット）や回線種別で絞り込めます

Lighthouse を実行しないため 1 秒かからずに返り、多数の URL の監視や、CLI・AI エージェントから Core Web Vitals を取得する用途に向いています。Google Cloud で Chrome UX Report API を有効にして API キーを発行して使ってください。データがない URL は 404 になるので、`is_not_found()` で判定してオリジンの値に切り替えてください。
