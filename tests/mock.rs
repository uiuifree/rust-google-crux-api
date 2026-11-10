use google_crux_api::metric::{self, Rating};
use google_crux_api::{
    CruxApi, EffectiveConnectionType, FormFactor, GoogleApiError, QueryHistoryRequest,
    QueryHistoryResponse, QueryRecordRequest, QueryRecordResponse, Record, Target,
};
use httpmock::prelude::*;
use serde_json::json;
use std::error::Error;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const RECORD: &str = include_str!("fixtures/query_record.json");
const HISTORY: &str = include_str!("fixtures/query_history_record.json");

#[tokio::test]
async fn query_record_parses_metrics() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/records:queryRecord")
                .header("x-goog-api-key", "test-key")
                .matches(|request| request.query_params.as_ref().is_none_or(Vec::is_empty))
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
                .json_body(json!({"url": "https://example.com/#top", "collectionPeriodCount": 3}));
            then.status(200).body(HISTORY);
        })
        .await;

    let api = CruxApi::with_api_key("k").with_base_url(server.base_url());
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

fn record(metrics: serde_json::Value) -> Record {
    serde_json::from_value(json!({ "metrics": metrics })).unwrap()
}

#[test]
fn passes_needs_both_lcp_and_cls() {
    let lcp = json!({"percentiles": {"p75": 1000}});
    let cls = json!({"percentiles": {"p75": "0.01"}});
    let inp = json!({"percentiles": {"p75": 100}});

    let only_lcp = record(json!({ "largest_contentful_paint": lcp }));
    assert_eq!(only_lcp.core_web_vitals().passes(), None);

    let no_lcp = record(json!({
        "cumulative_layout_shift": cls,
        "interaction_to_next_paint": inp,
    }));
    assert_eq!(no_lcp.core_web_vitals().passes(), None);

    let poor_inp = record(json!({
        "largest_contentful_paint": lcp,
        "cumulative_layout_shift": cls,
        "interaction_to_next_paint": {"percentiles": {"p75": 900}},
    }));
    assert_eq!(poor_inp.core_web_vitals().passes(), Some(false));
}

#[test]
fn metrics_without_data_give_none() {
    assert_eq!(metric::thresholds(metric::ROUND_TRIP_TIME), None);
    assert_eq!(metric::rate(metric::ROUND_TRIP_TIME, 100.0), None);

    let record = record(json!({ "largest_contentful_paint": {"histogram": []} }));
    assert_eq!(record.p75(metric::FIRST_CONTENTFUL_PAINT), None);
    assert_eq!(record.core_web_vitals().lcp, None);
}

#[test]
fn unknown_key_values_do_not_fail_the_response() {
    let response: QueryRecordResponse = serde_json::from_value(json!({
        "record": {"key": {"origin": "https://a.example", "formFactor": "WATCH", "effectiveConnectionType": "5G"}}
    }))
    .unwrap();
    assert_eq!(response.record.key.form_factor, None);
    assert_eq!(response.record.key.effective_connection_type, None);

    let response: QueryRecordResponse = serde_json::from_value(json!({
        "record": {"key": {"formFactor": "TABLET", "effectiveConnectionType": "slow-2G"}}
    }))
    .unwrap();
    assert_eq!(response.record.key.form_factor, Some(FormFactor::Tablet));
    assert_eq!(
        response.record.key.effective_connection_type,
        Some(EffectiveConnectionType::Slow2G)
    );
}

#[test]
fn malformed_values_are_rejected() {
    let cases = [
        r#"{"record":{"key":{"formFactor":"#,
        r#"{"record":{"metrics":{"m":{"fractions":1}}}}"#,
        r#"{"record":{"metrics":{"m":{"histogram":[{"start":"#,
    ];
    for body in cases {
        assert!(
            serde_json::from_str::<QueryRecordResponse>(body).is_err(),
            "{body}"
        );
    }
    let body = r#"{"record":{"metrics":{"m":{"percentilesTimeseries":{"p75s":1}}}}}"#;
    assert!(serde_json::from_str::<QueryHistoryResponse>(body).is_err());
}

#[test]
fn client_debug_hides_the_key() {
    let api = CruxApi::with_api_key("SECRET");
    let debug = format!("{api:?}");
    assert!(!debug.contains("SECRET"), "{debug}");
    assert!(debug.contains("<redacted>"), "{debug}");
}

#[test]
fn requests_serialize_every_option() {
    let cases = [
        (FormFactor::AllFormFactors, "ALL_FORM_FACTORS"),
        (FormFactor::Phone, "PHONE"),
        (FormFactor::Desktop, "DESKTOP"),
        (FormFactor::Tablet, "TABLET"),
    ];
    for (form_factor, wire) in cases {
        let request = QueryHistoryRequest::origin("https://a.example").form_factor(form_factor);
        assert_eq!(request.to_json()["formFactor"], wire);
    }

    let cases = [
        (EffectiveConnectionType::FourG, "4G"),
        (EffectiveConnectionType::ThreeG, "3G"),
        (EffectiveConnectionType::TwoG, "2G"),
        (EffectiveConnectionType::Slow2G, "slow-2G"),
        (EffectiveConnectionType::Offline, "offline"),
    ];
    for (ect, wire) in cases {
        let request = QueryRecordRequest::url("https://a.example/").effective_connection_type(ect);
        assert_eq!(request.to_json()["effectiveConnectionType"], wire);
    }

    let request = QueryRecordRequest::origin("https://a.example")
        .metrics(["a", "b"])
        .metric("a");
    assert_eq!(request.to_json()["metrics"], json!(["a", "b"]));

    let request = QueryHistoryRequest::new(Target::Url("https://a.example/".into()))
        .metrics(["a", "b"])
        .metric("a");
    assert_eq!(request.to_json()["metrics"], json!(["a", "b"]));
    assert_eq!(
        request.target(),
        &Target::Url("https://a.example/".to_string())
    );
}

#[test]
fn error_helpers() {
    let invalid_key = GoogleApiError::HttpStatus(
        400,
        r#"{"error":{"status":"INVALID_ARGUMENT","details":[{"locale":"en-US"},{"reason":"API_KEY_INVALID"}]}}"#.into(),
    );
    assert_eq!(invalid_key.reason().as_deref(), Some("API_KEY_INVALID"));
    assert!(!invalid_key.is_timeout());
    assert!(invalid_key.source().is_none());
    assert!(invalid_key.to_string().starts_with("http status 400: "));

    let not_json = GoogleApiError::HttpStatus(502, "<html>".into());
    assert_eq!(not_json.reason(), None);

    let no_error = GoogleApiError::HttpStatus(500, "{}".into());
    assert_eq!(no_error.reason(), None);

    let status_only = GoogleApiError::HttpStatus(404, r#"{"error":{"status":"NOT_FOUND"}}"#.into());
    assert_eq!(status_only.reason().as_deref(), Some("NOT_FOUND"));

    let no_status = GoogleApiError::HttpStatus(500, r#"{"error":{}}"#.into());
    assert_eq!(no_status.reason(), None);

    let invalid = GoogleApiError::InvalidArgument("x".into());
    assert_eq!(invalid.status(), None);
    assert_eq!(invalid.reason(), None);
    assert!(invalid.source().is_none());
    assert_eq!(invalid.to_string(), "invalid argument: x");
}

async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn connection_errors_do_not_contain_the_key() {
    let api = CruxApi::with_api_key("SECRET")
        .with_base_url(format!("http://127.0.0.1:{}", free_port().await));
    let error = api
        .query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::Connection(_)));
    assert!(!error.is_timeout());
    assert!(error.source().is_some());
    let message = format!("{error} {error:?}");
    assert!(message.starts_with("connection error: "), "{message}");
    assert!(!message.contains("SECRET"), "{message}");
}

#[tokio::test]
async fn timeout_is_reported() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/records:queryRecord");
            then.status(200)
                .delay(Duration::from_millis(500))
                .body(RECORD);
        })
        .await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let api = CruxApi::with_api_key("k")
        .with_base_url(server.base_url())
        .with_client(client);
    let error = api
        .query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap_err();
    assert!(error.is_timeout(), "{error}");
}

#[tokio::test]
async fn truncated_body_is_a_connection_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = [0; 4096];
        let _ = socket.read(&mut buffer).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n{")
            .await
            .unwrap();
    });

    let api = CruxApi::with_api_key("k").with_base_url(format!("http://127.0.0.1:{port}"));
    let error = api
        .query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::Connection(_)), "{error}");
}

#[tokio::test]
async fn invalid_json_is_reported() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/records:queryRecord");
            then.status(200).body("<html>");
        })
        .await;

    let api = CruxApi::with_api_key("k").with_base_url(server.base_url());
    let error = api
        .query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::JsonParse(_)));
    assert!(error.source().is_some());
    assert!(error.to_string().starts_with("json parse error: "));
}

#[tokio::test]
async fn invalid_base_url_is_rejected() {
    let api = CruxApi::with_api_key("k").with_base_url("not a url");
    let error = api
        .query_record(&QueryRecordRequest::origin("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::InvalidArgument(_)));
}

#[tokio::test]
async fn history_raw_returns_untyped_json() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/records:queryHistoryRecord");
            then.status(200).body(HISTORY);
        })
        .await;

    let api = CruxApi::with_api_key("k").with_base_url(server.base_url());
    let value = api
        .query_history_record_raw(&QueryHistoryRequest::origin("https://example.com"))
        .await
        .unwrap();
    assert_eq!(
        value["record"]["collectionPeriods"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let error = api
        .query_history_record_raw(&QueryHistoryRequest::origin("example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::InvalidArgument(_)));
    let error = api
        .query_record_raw(&QueryRecordRequest::origin("example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, GoogleApiError::InvalidArgument(_)));
}
