# lumia-core

Fundamental abstractions and types for Lumia.

This crate contains the runtime pieces of the framework: the `Server`, the
`Router`, the `Context` passed to handlers (including the typed `Context<T>`
that carries a deserialized request body) and the `Response` they return.

The `Server` also serves an OpenAPI 3.0 document, built from the operations
declared with `#[openapi(...)]`, at `/openapi.json` by default, plus a Scalar
API reference at `/docs`. Most applications depend on the `lumia` crate
instead, which re-exports everything here along with the attribute and derive
macros.
