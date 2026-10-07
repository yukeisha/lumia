//! Fundamental abstractions and types for Lumia.
//!
//! This crate contains the runtime pieces of the framework: the [`Server`], the
//! [`Router`], the [`Context`] passed to handlers and the [`Response`] they
//! return. Most applications depend on the `lumia` crate instead, which
//! re-exports everything here along with the [`route`] attribute macro.
//!
//! [`route`]: https://docs.rs/lumia-macros

mod context;
mod error;
mod request;
mod response;
mod router;
mod server;
mod util;

pub use context::Context;
pub use error::{Error, Result};
pub use request::Request;
pub use response::{Html, IntoResponse, Json, Response, Text};
pub use router::{BoxFuture, Route, RouteMatch, Router};
pub use server::Server;

pub use bytes::Bytes;
pub use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
