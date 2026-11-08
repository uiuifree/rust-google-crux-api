use std::fmt::{Debug, Display, Formatter};

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
/// ```
pub enum GoogleApiError {
    /// Failed to connect, send the request, or read the response body.
    Connection(String),
    /// The response body is not the expected JSON. Holds the parser message.
    JsonParse(String),
    /// The API returned an error status. Holds the HTTP status code and the response body.
    ///
    /// Common cases: `404` when the Chrome UX Report has no data for the URL or
    /// origin (see [`is_not_found`](GoogleApiError::is_not_found)), `400` for an
    /// invalid request, `403` when the API key is missing or invalid, and `429`
    /// when the quota is used up.
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

impl Debug for GoogleApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl std::error::Error for GoogleApiError {}
