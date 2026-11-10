use serde_json::Value;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Error type returned by all API calls.
///
/// The variants tell you where the call failed: before sending
/// ([`InvalidArgument`](Self::InvalidArgument)), while sending
/// ([`Connection`](Self::Connection)), in Google's answer
/// ([`HttpStatus`](Self::HttpStatus)), or while reading the answer
/// ([`JsonParse`](Self::JsonParse)).
///
/// # Example
///
/// ```
/// use google_crux_api::GoogleApiError;
///
/// let error = GoogleApiError::HttpStatus(404, r#"{"error":{"code":404,"message":"chrome ux report data not found","status":"NOT_FOUND"}}"#.into());
/// assert!(error.is_not_found());
/// assert_eq!(error.status(), Some(404));
/// assert_eq!(error.reason().as_deref(), Some("NOT_FOUND"));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum GoogleApiError {
    /// Failed to connect, send the request, or read the response body.
    Connection(reqwest::Error),
    /// The response body is not the expected JSON.
    JsonParse(serde_json::Error),
    /// The API returned an error status. Holds the HTTP status code and the response body.
    ///
    /// Common cases: `404` when the Chrome UX Report has no data for the URL or
    /// origin (see [`is_not_found`](GoogleApiError::is_not_found)), `400` for an
    /// invalid request or API key, `403` when the API key is missing or the API
    /// is not enabled, and `429` when the quota is used up.
    HttpStatus(u16, String),
    /// The request was rejected before sending.
    InvalidArgument(String),
}

impl GoogleApiError {
    /// Returns the HTTP status code for [`HttpStatus`](Self::HttpStatus), `None` otherwise.
    pub fn status(&self) -> Option<u16> {
        match self {
            GoogleApiError::HttpStatus(status, _) => Some(*status),
            _ => None,
        }
    }

    /// `true` when the API answered `404`, which means the Chrome UX Report has
    /// no data for the requested URL or origin with the given filters. This is
    /// common for pages with little traffic and is usually not a real error.
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// `true` when the request timed out.
    pub fn is_timeout(&self) -> bool {
        matches!(self, GoogleApiError::Connection(e) if e.is_timeout())
    }

    /// Machine-readable cause from the error body of
    /// [`HttpStatus`](Self::HttpStatus): the `reason` of the first detail that
    /// has one (`API_KEY_INVALID`), otherwise the status (`NOT_FOUND`).
    pub fn reason(&self) -> Option<String> {
        let GoogleApiError::HttpStatus(_, body) = self else {
            return None;
        };
        let body: Value = serde_json::from_str(body).ok()?;
        let error = body.get("error")?;
        let reason = error
            .get("details")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find_map(|detail| detail.get("reason"))
            .or_else(|| error.get("status"))?;
        reason.as_str().map(str::to_string)
    }
}

impl Display for GoogleApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GoogleApiError::Connection(e) => write!(f, "connection error: {}", e),
            GoogleApiError::JsonParse(e) => write!(f, "json parse error: {}", e),
            GoogleApiError::HttpStatus(status, body) => {
                write!(f, "http status {}: {}", status, body)
            }
            GoogleApiError::InvalidArgument(e) => write!(f, "invalid argument: {}", e),
        }
    }
}

impl Error for GoogleApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            GoogleApiError::Connection(e) => Some(e),
            GoogleApiError::JsonParse(e) => Some(e),
            GoogleApiError::HttpStatus(..) | GoogleApiError::InvalidArgument(_) => None,
        }
    }
}
