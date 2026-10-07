use std::error::Error as StdError;
use std::fmt;

use http::StatusCode;
use serde_json::json;

use crate::response::{IntoResponse, Response};

/// Convenience alias used throughout Lumia.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error that can be returned from a handler and turned into a response.
///
/// `Error` implements [`IntoResponse`], so handlers may return `Result<T, Error>`
/// and have failures rendered as a JSON body such as `{"error": "..."}`.
#[derive(Debug)]
pub struct Error {
    status: StatusCode,
    message: String,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

impl Error {
    /// Creates an error with an explicit status code and message.
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a `400 Bad Request` error.
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// Creates a `404 Not Found` error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    /// Creates a `500 Internal Server Error`.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    /// Attaches the underlying cause of this error.
    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// The status code that will be sent to the client.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The human readable message that will be sent to the client.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn StdError + 'static))
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::bad_request("invalid JSON body").with_source(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::internal("I/O error").with_source(error)
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status;
        let body = serde_json::to_vec(&json!({ "error": self.message }))
            .unwrap_or_else(|_| b"{\"error\":\"internal error\"}".to_vec());

        Response::new(body).with_status(status).with_header(
            http::header::CONTENT_TYPE.as_str(),
            "application/json; charset=utf-8",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_status_and_message() {
        let error = Error::bad_request("nope");
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error.message(), "nope");
        assert_eq!(error.to_string(), "nope");
    }

    #[test]
    fn renders_as_json() {
        let response = Error::not_found("missing").into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.body(), br#"{"error":"missing"}"#);
    }
}
