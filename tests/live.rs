//! Tests against the real Chrome UX Report API.
//!
//! They are ignored by default because they need an API key. Run them with:
//!
//! ```bash
//! CRUX_API_KEY=... cargo test --test live -- --ignored
//! ```

use google_crux_api::metric;
use google_crux_api::{CruxApi, FormFactor, QueryHistoryRequest, QueryRecordRequest};

// An origin that always has CrUX data
const ORIGIN: &str = "https://www.google.com";

fn api() -> CruxApi {
    let key = std::env::var("CRUX_API_KEY").expect("CRUX_API_KEY is not set");
    CruxApi::with_api_key(key)
}

#[tokio::test]
#[ignore]
async fn query_record_origin() {
    let response = api()
        .query_record(&QueryRecordRequest::origin(ORIGIN))
        .await
        .unwrap();
    let record = &response.record;
    assert_eq!(record.key.origin.as_deref(), Some(ORIGIN));
    assert!(record.collection_period.is_some());

    let vitals = record.core_web_vitals();
    let lcp = vitals.lcp.expect("lcp");
    assert!(lcp.p75 > 0.0);
    assert!(lcp.good.is_some() && lcp.poor.is_some());
    // CLS comes as a string on the wire and must still parse
    let cls = vitals.cls.expect("cls");
    assert!((0.0..10.0).contains(&cls.p75), "{cls:?}");
    assert!(vitals.passes().is_some());

    // Fraction metric is present when no form factor is set
    let form_factors = record.metric(metric::FORM_FACTORS).expect("form_factors");
    assert!(form_factors.fractions.contains_key("phone"));
}

#[tokio::test]
#[ignore]
async fn query_record_with_filters() {
    let request = QueryRecordRequest::origin(ORIGIN)
        .form_factor(FormFactor::Phone)
        .metrics(metric::CORE_WEB_VITALS);
    let response = api().query_record(&request).await.unwrap();
    assert_eq!(response.record.key.form_factor, Some(FormFactor::Phone));
    for name in metric::CORE_WEB_VITALS {
        assert!(response.record.metric(name).is_some(), "{name}");
    }
    assert!(response
        .record
        .metric(metric::FIRST_CONTENTFUL_PAINT)
        .is_none());
}

#[tokio::test]
#[ignore]
async fn query_history_record_origin() {
    let request = QueryHistoryRequest::origin(ORIGIN)
        .metric(metric::LARGEST_CONTENTFUL_PAINT)
        .metric(metric::CUMULATIVE_LAYOUT_SHIFT)
        .collection_period_count(5);
    let response = api().query_history_record(&request).await.unwrap();
    let record = &response.record;
    assert_eq!(record.collection_periods.len(), 5);

    let lcp = record.p75_series(metric::LARGEST_CONTENTFUL_PAINT);
    assert_eq!(lcp.len(), 5);
    assert!(lcp.iter().any(|(_, v)| v.is_some()));
    assert!(lcp[0].0.last_date < lcp[4].0.last_date);

    let cls = record.p75_series(metric::CUMULATIVE_LAYOUT_SHIFT);
    assert!(cls.iter().flat_map(|(_, v)| v).all(|v| *v < 10.0));
}

#[tokio::test]
#[ignore]
async fn unknown_url_is_not_found() {
    let request = QueryRecordRequest::url("https://example.com/this-page-has-no-crux-data-1b2c3d");
    let error = api().query_record(&request).await.unwrap_err();
    assert!(error.is_not_found(), "{error}");
}
