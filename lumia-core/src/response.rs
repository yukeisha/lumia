use std::fmt::Debug;

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use serde::Serialize;

use crate::error::Error;

/// An outgoing HTTP response.
///
/// Build one directly with [`Response::json`], [`Response::text`] or
/// [`Response::new`], or let [`Context::Json`](crate::Context::Json) build it.
#[derive(Debug, Clone)]
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl Response {
    /// Creates a `200 OK` response with the given body.
    pub fn new(body: impl Into<Bytes>) -> Self {
        Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: body.into(),
        }
    }

    /// Creates an empty `200 OK` response.
    pub fn empty() -> Self {
        Self::new(Bytes::new())
    }

    /// Creates a `text/plain; charset=utf-8` response.
    pub fn text(body: impl Into<String>) -> Self {
        Self::new(body.into())
            .with_header(header::CONTENT_TYPE.as_str(), "text/plain; charset=utf-8")
    }

    /// Creates a `text/html; charset=utf-8` response.
    pub fn html(body: impl Into<String>) -> Self {
        Self::new(body.into())
            .with_header(header::CONTENT_TYPE.as_str(), "text/html; charset=utf-8")
    }

    /// Creates an `application/json; charset=utf-8` response.
    ///
    /// If the value cannot be serialized the response becomes a
    /// `500 Internal Server Error`.
    pub fn json<T: Serialize>(value: T) -> Self {
        match serde_json::to_vec(&value) {
            Ok(body) => Self::new(body).with_header(
                header::CONTENT_TYPE.as_str(),
                "application/json; charset=utf-8",
            ),
            Err(error) => Error::internal("failed to serialize response body")
                .with_source(error)
                .into_response(),
        }
    }

    /// Creates a `302 Found` response pointing at `location`.
    pub fn redirect(location: impl AsRef<str>) -> Self {
        Self::empty()
            .with_status(StatusCode::FOUND)
            .with_header(header::LOCATION.as_str(), location.as_ref())
    }

    /// Replaces the status code.
    pub fn with_status(mut self, status: impl Into<StatusCode>) -> Self {
        self.status = status.into();
        self
    }

    /// Adds a header, replacing any previous value for the same name.
    pub fn with_header<N, V>(mut self, name: N, value: V) -> Self
    where
        N: TryInto<HeaderName>,
        N::Error: Debug,
        V: TryInto<HeaderValue>,
        V::Error: Debug,
    {
        let name = name.try_into().expect("invalid header name");
        let value = value.try_into().expect("invalid header value");
        self.headers.insert(name, value);
        self
    }

    /// The response status code.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// All response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// A single response header.
    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.headers.get(name)
    }

    /// The response body.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Consumes the response and returns its body.
    pub fn into_body(self) -> Bytes {
        self.body
    }
}

/// Conversion into a [`Response`].
///
/// Implemented for [`Response`] itself, for common body types such as `&str`
/// and `String`, for [`Json`]/[`Text`]/[`Html`] wrappers, for [`Error`] and for
/// `Result<T, E>` where both `T` and `E` implement `IntoResponse`.
pub trait IntoResponse {
    /// Converts `self` into a [`Response`].
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response::empty()
    }
}

impl IntoResponse for &str {
    fn into_response(self) -> Response {
        Response::text(self)
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Response::text(self)
    }
}

impl IntoResponse for std::borrow::Cow<'_, str> {
    fn into_response(self) -> Response {
        Response::text(self.into_owned())
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for &[u8] {
    fn into_response(self) -> Response {
        Response::new(self.to_vec())
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        Response::empty().with_status(self)
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(error) => error.into_response(),
        }
    }
}

/// A JSON response body.
///
/// Handlers may return `Json<T>` directly:
///
/// ```
/// use lumia_core::{Json, Response};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Greeting {
///     message: String,
/// }
///
/// fn greet() -> Json<Greeting> {
///     Json(Greeting { message: "Hello".to_owned() })
/// }
/// # let _: Response = lumia_core::IntoResponse::into_response(greet());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        Response::json(self.0)
    }
}

/// A `text/plain` response body.
#[derive(Debug, Clone, Copy)]
pub struct Text<T>(pub T);

impl<T: Into<String>> IntoResponse for Text<T> {
    fn into_response(self) -> Response {
        Response::text(self.0)
    }
}

/// A `text/html` response body.
#[derive(Debug, Clone, Copy)]
pub struct Html<T>(pub T);

impl<T: Into<String>> IntoResponse for Html<T> {
    fn into_response(self) -> Response {
        Response::html(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_sets_content_type() {
        let response = Response::json(serde_json::json!({ "ok": true }));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .header("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json; charset=utf-8")
        );
        assert_eq!(response.body(), br#"{"ok":true}"#);
    }

    #[test]
    fn text_sets_content_type() {
        let response = Response::text("hi");
        assert_eq!(
            response
                .header("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(response.body(), b"hi");
    }

    #[test]
    fn result_maps_both_sides() {
        let ok: Result<&str, Error> = Ok("fine");
        assert_eq!(ok.into_response().body(), b"fine");

        let err: Result<&str, Error> = Err(Error::not_found("gone"));
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn wrappers_render_as_expected() {
        assert_eq!(Json(1).into_response().body(), b"1");
        assert_eq!(Text("a").into_response().body(), b"a");
        assert_eq!(Html("<p>a</p>").into_response().body(), b"<p>a</p>");
    }
}
