//! OpenAPI 3.0 generation for Lumia.
//!
//! Handlers annotated with `#[openapi(...)]` describe themselves as an
//! [`Operation`], and a [`Server`](https://docs.rs/lumia) collects those
//! operations into an OpenAPI document served at `/openapi.json`.
//!
//! Application types opt in with `#[derive(Schema)]` (request bodies) and
//! `#[derive(Response)]` (response bodies), or by implementing
//! [`OpenApiSchema`] and [`ApiResponse`] directly.

mod docs;
mod document;
mod operation;
mod response;
mod schema;

pub use docs::scalar_html;
pub use document::OpenApi;
pub use operation::{Operation, RequestBody, ResponseBody};
pub use response::{
    ApiResponse, InternalErrorResponse, NotFoundErrorResponse, ValidationErrorResponse,
};
pub use schema::{OpenApiSchema, Schema, enum_schema, object_schema};
