//! Batteries-included REST API framework for Rust.
//!
//! Lumia is built around three ideas:
//!
//! * handlers are plain `async fn`s annotated with [`#[route(...)]`](route),
//! * handlers receive a [`Context`] and return anything implementing
//!   [`IntoResponse`],
//! * [`Server`] ties the routes together.
//!
//! ```no_run
//! use lumia::prelude::*;
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut server = Server::new();
//!     server.route(greet);
//!     server.run("0.0.0.0:3000").await.unwrap();
//! }
//!
//! #[route(GET "/")]
//! async fn greet(ctx: Context) -> Response {
//!     ctx.Json(serde_json::json!({
//!         "message": "Hello, World!"
//!     }))
//! }
//! ```
//!
//! Routes can capture path parameters and read the query string:
//!
//! ```no_run
//! use lumia::prelude::*;
//!
//! #[route(GET "/users/{id}")]
//! async fn show_user(ctx: Context) -> Response {
//!     let id = ctx.param("id").unwrap_or("unknown").to_owned();
//!     ctx.Json(serde_json::json!({ "id": id, "expand": ctx.query("expand") }))
//! }
//! ```
//!
//! Forgetting to handle a route is not a problem: unmatched paths get a `404`
//! and a known path with an unknown method gets a `405` with an `Allow` header.

pub use lumia_core::*;
pub use lumia_macros::route;

pub use serde_json;

/// Everything needed to write a Lumia application.
///
/// ```no_run
/// use lumia::prelude::*;
/// # #[route(GET "/")]
/// # async fn handler(ctx: Context) -> Response { ctx.Text("ok") }
/// ```
pub mod prelude {
    pub use crate::{
        BoxFuture, Bytes, Context, Error, HeaderMap, HeaderName, HeaderValue, Html, IntoResponse,
        Json, Method, Request, Response, Result, Route, RouteMatch, Router, Server, StatusCode,
        Text, Uri, route,
    };
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
}
