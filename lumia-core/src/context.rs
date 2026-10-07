use http::{HeaderMap, Method, Uri};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::request::Request;
use crate::response::Response;

/// The request context passed to every handler.
///
/// A `Context` gives read access to the incoming request and small helpers for
/// building responses, such as [`Context::Json`], [`Context::Text`] and
/// [`Context::Html`].
///
/// Handlers receive it as their single argument:
///
/// ```text
/// #[route(GET "/users/{id}")]
/// async fn show(ctx: Context) -> Response {
///     let id = ctx.param("id").unwrap_or("unknown");
///     ctx.Json(serde_json::json!({ "id": id }))
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Context {
    request: Request,
}

impl Context {
    pub(crate) fn new(request: Request) -> Self {
        Self { request }
    }

    /// The underlying request.
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Consumes the context, returning the underlying request.
    pub fn into_request(self) -> Request {
        self.request
    }

    /// The request method.
    pub fn method(&self) -> &Method {
        self.request.method()
    }

    /// The full request URI, including the query string.
    pub fn uri(&self) -> &Uri {
        self.request.uri()
    }

    /// The request path, without the query string.
    pub fn path(&self) -> &str {
        self.request.path()
    }

    /// The raw query string, if any.
    pub fn query_string(&self) -> Option<&str> {
        self.request.query_string()
    }

    /// All path parameters captured by the route template.
    pub fn params(&self) -> &std::collections::HashMap<String, String> {
        self.request.params()
    }

    /// A single path parameter, for example `id` in `/users/{id}`.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.request.param(name)
    }

    /// All parsed query string values.
    pub fn queries(&self) -> &std::collections::HashMap<String, String> {
        self.request.queries()
    }

    /// A single query string value, for example `q` in `/?q=rust`.
    pub fn query(&self, name: &str) -> Option<&str> {
        self.request.query(name)
    }

    /// All request headers.
    pub fn headers(&self) -> &HeaderMap {
        self.request.headers()
    }

    /// A single request header.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    }

    /// The raw request body.
    pub fn body(&self) -> &[u8] {
        self.request.body()
    }

    /// The request body interpreted as UTF-8, replacing invalid sequences.
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.request.body())
    }

    /// Deserializes the request body as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(self.request.body()).map_err(Error::from)
    }

    /// Builds a JSON response.
    ///
    /// The capital `J` matches the syntax used by the Lumia examples
    /// (`ctx.Json(...)`).
    #[allow(non_snake_case)]
    pub fn Json<T: Serialize>(&self, value: T) -> Response {
        Response::json(value)
    }

    /// Builds a `text/plain` response.
    #[allow(non_snake_case)]
    pub fn Text(&self, body: impl Into<String>) -> Response {
        Response::text(body)
    }

    /// Builds a `text/html` response.
    #[allow(non_snake_case)]
    pub fn Html(&self, body: impl Into<String>) -> Response {
        Response::html(body)
    }
}
