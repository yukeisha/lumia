# lumia-macros

Attribute macros for [Lumia](../lumia).

## `#[route]`

Declares an HTTP route. The annotated function keeps its body but is renamed,
while the original name becomes a value implementing `lumia::Route` that
carries the HTTP method and path:

```rust
use lumia::prelude::*;

#[route(GET "/")]
async fn greet(ctx: Context) -> Response {
    ctx.Json(serde_json::json!({ "message": "Hello, World!" }))
}

let mut server = Server::new();
server.route(greet);
```

The method may be omitted, in which case `GET` is assumed:

```rust
#[route("/health")]
async fn health(_ctx: Context) -> &'static str {
    "ok"
}
```

Supported methods are `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`,
`OPTIONS`, `TRACE` and `CONNECT`, written in any case.

Paths may contain `{name}` parameters, a single trailing `*name` catch-all
segment, and are read with `Context::param`.

Handlers must be `async fn`s with exactly one `Context` argument and no
generics.

> The macro defines a unit struct named after the handler in the same scope, so
> avoid binding a local variable with the exact name of a handler declared in
> the same module.

Handlers may declare a typed body with `Context<T>`; the JSON body is
deserialized before the handler runs and exposed as `ctx.req`:

```rust
#[route(POST "/todos")]
async fn create(ctx: Context<CreateTodoRequest>) -> Response {
    ctx.Json(serde_json::json!({ "title": ctx.req.title }))
}
```

## `#[openapi]`

Describes a route for the generated OpenAPI document. It may be written after
or before `#[route(...)]`.

```rust
#[route(POST "/todos")]
#[openapi(
    summary = "Create a new todo",
    tag = "Todo",
    request = CreateTodoRequest,
    responses = (CreateTodoResponse, ValidationErrorResponse),
)]
async fn create(ctx: Context<CreateTodoRequest>) -> Response { /* ... */ }
```

Supported keys are `summary`, `description`, `tag` (repeatable), `tags`,
`operation_id`, `deprecated`, `request` and `responses`.

## `#[derive(Schema)]`

Derives an OpenAPI JSON schema for a struct (object) or an enum with unit
variants (string enum). Honours `#[serde(rename = "...")]`,
`#[serde(rename_all = "...")]` and `#[serde(skip)]`.

## `#[derive(Response)]`

Derives a response type: a `builder()`, an `IntoResponse` implementation and
`ApiResponse` metadata.

```rust
#[derive(Serialize, Response)]
#[response(status = 201, description = "Todo created")]
struct CreateTodoResponse {
    title: String,
}
```
