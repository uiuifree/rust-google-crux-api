use google_crux_api::metric::{self, Rating};
use google_crux_api::{
    CruxApi, FormFactor, GoogleApiError, QueryHistoryRequest, QueryRecordRequest,
};
use httpmock::prelude::*;
use serde_json::json;

const RECORD: &str = include_str!("fixtures/query_record.json");
const HISTORY: &str = include_str!("fixtures/query_history_record.json");

#[tokio::test]
async fn query_record_parses_metrics() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/records:queryRecord")
                .query_param("key", "test-key")
                .json_body(json!({"origin": "https://example.com", "formFactor": "PHONE"}));
            then.status(200)
                .header("content-type", "application/json")
                .body(RECORD);
        })
        .await;

    let api = CruxApi::with_api_key("test-key").with_base_url(server.base_url());
    let response = api
        .query_record(
            &QueryRecordRequest::origin("https://example.com").form_factor(FormFactor::Phone),
        )
        .await
        .unwrap();
    mock.assert_async().await;

    let record = &response.record;
    assert_eq!(record.key.origin.as_deref(), Some("https://example.com"));
    assert_eq!(record.key.form_factor, Some(FormFactor::Phone));
    assert_eq!(
        record.collection_period.unwrap().last_date.to_string(),
        "2026-10-31"
    );

    // CLS comes as strings on the wire
    assert_eq!(record.p75(metric::CUMULATIVE_LAYOUT_SHIFT), Some(0.03));
    let cls = record.metric(metric::CUMULATIVE_LAYOUT_SHIFT).unwrap();
    assert_eq!(cls.histogram[1].start, Some(0.1));
    assert_eq!(cls.histogram[2].end, None);

    assert_eq!(
        record.p75(metric::LARGEST_CONTENTFUL_PAINT_IMAGE_RESOURCE_LOAD_DELAY),
        Some(412.0)
    );
    let nav = record.metric(metric::NAVIGATION_TYPES).unwrap();
    assert_eq!(nav.fractions["back_forward_cache"], 0.1702);
    assert!(nav.p75().is_none());

    let vitals = record.core_web_vitals();
    let lcp = vitals.lcp.unwrap();
    assert_eq!(lcp.p75, 2611.0);
    assert_eq!(lcp.rating, Some(Rating::NeedsImprovement));
    assert_eq!(lcp.good, Some(0.7305));
    assert_eq!(lcp.poor, Some(0.0983));
    assert_eq!(vitals.inp.unwrap().rating, Some(Rating::Good));
    assert_eq!(vitals.cls.unwrap().rating, Some(Rating::Good));
    assert_eq!(vitals.ttfb.unwrap().rating, Some(Rating::NeedsImprovement));
    assert_eq!(vitals.passes(), Some(false));
}

#[tokio::test]
async fn query_history_record_parses_series() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/records:queryHistoryRecord")
                .header("authorization", "Bearer token-1")
                .json_body(json!({"url": "https://example.com/#top", "collectionPeriodCount": 3}));
            then.status(200).body(HISTORY);
        })
        .await;

    let api = CruxApi::with_access_token("token-1").with_base_url(server.base_url());
    let response = api
        .query_history_record(
            &QueryHistoryRequest::url("https://example.com/#top").collection_period_count(3),
        )
        .await
        .unwrap();
    mock.assert_async().await;

    let normalization = response.url_normalization_details.unwrap();
    assert_eq!(
        normalization.normalized_url.as_deref(),
        Some("https://example.com/")
    );

    let record = &response.record;
    assert_eq!(record.collection_periods.len(), 3);

    let lcp: Vec<Option<f64>> = record
        .p75_series(metric::LARGEST_CONTENTFUL_PAINT)
        .into_iter()
        .map(|(_, v)| v)
        .collect();
    assert_eq!(lcp, [Some(2810.0), Some(2702.0), Some(2488.0)]);

    let cls = record.p75_series(metric::CUMULATIVE_LAYOUT_SHIFT);
    assert_eq!(cls[1].1, None);
    assert_eq!(cls[2].1, Some(0.05));
    assert_eq!(cls[2].0.first_date.to_string(), "2026-10-04");

    let bins = &record
        .metric(metric::CUMULATIVE_LAYOUT_SHIFT)
        .unwrap()
        .histogram_timeseries;
    assert_eq!(bins[0].densities, [Some(0.81), None, Some(0.86)]);

    let tablet = &record
        .metric(metric::FORM_FACTORS)
        .unwrap()
        .fraction_timeseries["tablet"];
    assert_eq!(tablet.fractions, [Some(0.02), Some(0.02), None]);

    // A metric that was not returned gives one None per period
    let inp = record.p75_series(metric::INTERACTION_TO_NEXT_PAINT);
    assert_eq!(inp.len(), 3);
    assert!(inp.iter().all(|(_, v)| v.is_none()));
}

#[tokio::test]
async fn not_found_is_reported() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/records:queryRecord");
            then.status(404).body(
                r#"{"error":{"code":404,"message":"chrome ux report data not found","status":"NOT_FOUND"}}"#,
            );
        })
        .await;

    let api = CruxApi::with_api_key("k").with_base_url(server.base_url());
    let error = api
        .query_record(&QueryRecordRequest::url("https://example.com/rare-page"))
        .await
        .unwrap_err();
    assert!(error.is_not_found());
}

#[tokio::test]
async fn invalid_arguments_are_not_sent() {
    let api = CruxApi::with_api_key("k").with_base_url("http://127.0.0.1:9");
    let error = api
        .query_record(&QueryRecordRequest::origin("example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::InvalidArgument(_)));

    let error = api
        .query_history_record(
            &QueryHistoryRequest::origin("https://example.com").collection_period_count(100),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::InvalidArgument(_)));
}

#[tokio::test]
async fn raw_returns_untyped_json() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/records:queryRecord");
            then.status(200).body(RECORD);
        })
        .await;

    let api = CruxApi::with_api_key("k").with_base_url(server.base_url());
    let value = api
        .query_record_raw(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap();
    assert_eq!(
        value["record"]["metrics"]["cumulative_layout_shift"]["percentiles"]["p75"],
        "0.03"
    );
}

#[test]
fn passes_ignores_missing_inp() {
    let response: google_crux_api::QueryRecordResponse = serde_json::from_value(json!({
        "record": {
            "key": {"url": "https://example.com/"},
            "metrics": {
                "largest_contentful_paint": {"percentiles": {"p75": 2000}},
                "cumulative_layout_shift": {"percentiles": {"p75": "0.02"}}
            }
        }
    }))
    .unwrap();
    assert_eq!(response.record.core_web_vitals().passes(), Some(true));

    let empty = google_crux_api::Record::default();
    assert_eq!(empty.core_web_vitals().passes(), None);
}

#[tokio::test]
async fn sends_quota_project() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/records:queryRecord")
                .query_param("key", "k")
                .header("x-goog-user-project", "my-project");
            then.status(200).body(RECORD);
        })
        .await;
    let api = CruxApi::with_api_key("k")
        .with_quota_project("my-project")
        .with_base_url(server.base_url());
    api.query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap();
    mock.assert_async().await;
}
