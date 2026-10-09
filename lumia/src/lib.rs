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
//! Handlers can declare a typed request body with `Context<T>`, and describe
//! themselves with `#[openapi(...)]` so the server can serve an OpenAPI
//! document at `/openapi.json`:
//!
//! ```no_run
//! use lumia::prelude::*;
//!
//! #[derive(Serialize, Deserialize, Schema)]
//! struct CreateTodoRequest {
//!     title: String,
//! }
//!
//! #[derive(Serialize, Response)]
//! struct CreateTodoResponse {
//!     title: String,
//! }
//!
//! #[route(POST "/todos")]
//! #[openapi(
//!     summary = "Create a new todo",
//!     tag = "Todo",
//!     request = CreateTodoRequest,
//!     responses = (CreateTodoResponse, ValidationErrorResponse, InternalErrorResponse),
//! )]
//! async fn create(ctx: Context<CreateTodoRequest>) -> Response {
//!     CreateTodoResponse::builder().title(ctx.req.title).build().into_response()
//! }
//! ```
//!
//! Forgetting to handle a route is not a problem: unmatched paths get a `404`
//! and a known path with an unknown method gets a `405` with an `Allow` header.

pub use lumia_core::*;
pub use lumia_macros::{Response, Schema, openapi, route};
pub use lumia_openapi as openapi;
pub use lumia_openapi::*;

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
        ApiResponse, BoxFuture, Bytes, Context, Error, HeaderMap, HeaderName, HeaderValue, Html,
        InternalErrorResponse, IntoResponse, Json, Method, NotFoundErrorResponse, OpenApi, Request,
        Response, Result, Route, RouteMatch, Router, Schema, Server, StatusCode, Text, Uri,
        ValidationErrorResponse, openapi, route,
    };
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
}
