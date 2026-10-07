use std::collections::HashMap;

use bytes::Bytes;
use http::{HeaderMap, Method, Uri};

use crate::util::parse_query;

/// An incoming HTTP request as seen by a handler.
///
/// Handlers normally interact with the request through [`Context`](crate::Context),
/// which exposes read-only access to the same data.
#[derive(Debug, Clone)]
pub struct Request {
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    params: HashMap<String, String>,
    query: HashMap<String, String>,
    body: Bytes,
}

impl Request {
    pub(crate) fn new(
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        params: HashMap<String, String>,
        body: Bytes,
    ) -> Self {
        let query = parse_query(uri.query().unwrap_or_default());
        Self {
            method,
            uri,
            headers,
            params,
            query,
            body,
        }
    }

    /// The request method.
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// The request URI, including the query string.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// The request path, without the query string.
    pub fn path(&self) -> &str {
        self.uri.path()
    }

    /// The raw query string, if any.
    pub fn query_string(&self) -> Option<&str> {
        self.uri.query()
    }

    /// Path parameters captured by the route template.
    pub fn params(&self) -> &HashMap<String, String> {
        &self.params
    }

    /// A single path parameter.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    /// Parsed query string values.
    pub fn queries(&self) -> &HashMap<String, String> {
        &self.query
    }

    /// A single query string value.
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query.get(name).map(String::as_str)
    }

    /// All request headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The raw request body.
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
