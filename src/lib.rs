#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod error;
pub mod metric;
mod request;
pub mod response;

pub use error::GoogleApiError;
pub use request::{
    EffectiveConnectionType, FormFactor, QueryHistoryRequest, QueryRecordRequest, Target,
};
pub use response::{
    CollectionPeriod, CoreWebVitals, Date, HistoryRecord, Metric, MetricSummary,
    QueryHistoryResponse, QueryRecordResponse, Record, UrlNormalization,
};

/// Re-export of `reqwest`, for building a client to pass to
/// [`CruxApi::with_client`] without adding the dependency yourself.
pub use reqwest;

use reqwest::Url;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://chromeuxreport.googleapis.com/v1";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Client for the Chrome UX Report API.
///
/// The API needs an API key from a Google Cloud project with the Chrome UX
/// Report API enabled. OAuth access tokens are rejected by this API.
///
/// # Example
///
/// ```no_run
/// use google_crux_api::{CruxApi, FormFactor, QueryRecordRequest};
///
/// # async fn run() -> Result<(), google_crux_api::GoogleApiError> {
/// let api = CruxApi::with_api_key("YOUR_API_KEY");
/// let request = QueryRecordRequest::origin("https://example.com").form_factor(FormFactor::Phone);
/// let response = api.query_record(&request).await?;
///
/// let vitals = response.record.core_web_vitals();
/// println!("passes: {:?}, LCP p75: {:?}", vitals.passes(), vitals.lcp.map(|m| m.p75));
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CruxApi {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    timeout: Option<Duration>,
}

impl Debug for CruxApi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CruxApi")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl CruxApi {
    /// Client that sends an API key in the `X-Goog-Api-Key` header.
    /// Requests time out after 30 seconds.
    pub fn with_api_key<T: Into<String>>(key: T) -> Self {
        CruxApi {
            api_key: key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
            timeout: Some(DEFAULT_TIMEOUT),
        }
    }

    /// Uses the given HTTP client, for example one with a proxy. The 30 second
    /// default no longer applies, so set a timeout on the client if you want one.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self.timeout = None;
        self
    }

    /// Changes the API base URL (default `https://chromeuxreport.googleapis.com/v1`).
    /// Mainly for tests against a mock server.
    pub fn with_base_url<T: Into<String>>(mut self, base_url: T) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Latest 28-day aggregate for a URL or origin (`POST records:queryRecord`).
    ///
    /// # Errors
    ///
    /// * [`GoogleApiError::HttpStatus`] with `404` when there is no data
    ///   (check with [`GoogleApiError::is_not_found`]), or another status for
    ///   invalid requests, credentials or quota.
    /// * [`GoogleApiError::InvalidArgument`] when the URL or origin does not
    ///   start with `http://` or `https://`.
    /// * [`GoogleApiError::Connection`] on a network failure or timeout, or
    ///   [`GoogleApiError::JsonParse`] when the body is not the expected JSON.
    pub async fn query_record(
        &self,
        request: &QueryRecordRequest,
    ) -> Result<QueryRecordResponse, GoogleApiError> {
        request
            .target()
            .validate()
            .map_err(GoogleApiError::InvalidArgument)?;
        self.post("records:queryRecord", &request.to_json()).await
    }

    /// Weekly history of 28-day aggregates (`POST records:queryHistoryRecord`).
    ///
    /// Each period ends one week after the previous one, so consecutive
    /// periods overlap by 21 days.
    ///
    /// # Errors
    ///
    /// Same as [`query_record`](Self::query_record), plus
    /// [`GoogleApiError::InvalidArgument`] when `collection_period_count` is
    /// not between 1 and 40.
    pub async fn query_history_record(
        &self,
        request: &QueryHistoryRequest,
    ) -> Result<QueryHistoryResponse, GoogleApiError> {
        request
            .validate()
            .map_err(GoogleApiError::InvalidArgument)?;
        self.post("records:queryHistoryRecord", &request.to_json())
            .await
    }

    /// Same as [`query_record`](Self::query_record) but returns the JSON as is.
    pub async fn query_record_raw(
        &self,
        request: &QueryRecordRequest,
    ) -> Result<Value, GoogleApiError> {
        request
            .target()
            .validate()
            .map_err(GoogleApiError::InvalidArgument)?;
        self.post("records:queryRecord", &request.to_json()).await
    }

    /// Same as [`query_history_record`](Self::query_history_record) but returns the JSON as is.
    pub async fn query_history_record_raw(
        &self,
        request: &QueryHistoryRequest,
    ) -> Result<Value, GoogleApiError> {
        request
            .validate()
            .map_err(GoogleApiError::InvalidArgument)?;
        self.post("records:queryHistoryRecord", &request.to_json())
            .await
    }

    fn build_url(&self, method: &str) -> Result<Url, GoogleApiError> {
        Url::parse(&format!("{}/{}", self.base_url, method))
            .map_err(|e| GoogleApiError::InvalidArgument(format!("base url: {}", e)))
    }

    async fn post<T: DeserializeOwned>(
        &self,
        method: &str,
        body: &Value,
    ) -> Result<T, GoogleApiError> {
        let url = self.build_url(method)?;
        let mut request = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body);
        if let Some(timeout) = self.timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await.map_err(GoogleApiError::Connection)?;
        let status = response.status();
        let text = response.text().await.map_err(GoogleApiError::Connection)?;
        if !status.is_success() {
            return Err(GoogleApiError::HttpStatus(status.as_u16(), text));
        }
        serde_json::from_str(&text).map_err(GoogleApiError::JsonParse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_stays_out_of_the_url() {
        let url = CruxApi::with_api_key("k")
            .build_url("records:queryRecord")
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://chromeuxreport.googleapis.com/v1/records:queryRecord"
        );
    }

    #[test]
    fn rejects_bad_period_count() {
        let request =
            QueryHistoryRequest::origin("https://example.com").collection_period_count(41);
        assert!(request.validate().is_err());
        let request = QueryHistoryRequest::origin("https://example.com").collection_period_count(0);
        assert!(request.validate().is_err());
        let request =
            QueryHistoryRequest::origin("https://example.com").collection_period_count(40);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn rejects_origin_without_scheme() {
        let request = QueryRecordRequest::origin("example.com");
        assert!(request.target().validate().is_err());
    }
}
