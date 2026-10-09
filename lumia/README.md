# lumia

Batteries-included REST API framework for Rust.

## Quickstart

```rust
use lumia::prelude::*;

#[tokio::main]
async fn main() {
    let mut server = Server::new();
    server.route(greet);
    server.run("0.0.0.0:3000").await.unwrap();
}

#[route(GET "/")]
async fn greet(ctx: Context) -> Response {
    ctx.Json(serde_json::json!({
        "message": "Hello, World!"
    }))
}
```

```
$ curl http://127.0.0.1:3000/
{"message":"Hello, World!"}
```

A complete version of this program lives in [`examples/hello-world`](../examples/hello-world).

## Routes

`#[route(METHOD "/path")]` turns an `async fn` into a route that can be handed
straight to `Server::route`. The method defaults to `GET` when omitted.

| Template           | Example request | `ctx.param(...)`  |
| ------------------ | --------------- | ----------------- |
| `/users`           | `GET /users`    | —                 |
| `/users/{id}`      | `GET /users/7`  | `id` → `7`        |
| `/users/:id`       | `GET /users/7`  | `id` → `7`        |
| `/assets/*path`    | `GET /assets/a/b.png` | `path` → `a/b.png` |

Path parameters are percent-decoded, trailing slashes are ignored, and
unmatched paths return `404` while a known path with an unknown method returns
`405` with an `Allow` header.

> Because the macro defines a unit struct named after the handler, avoid
> binding a local variable with the exact name of a handler declared in the
> same module.

## Handlers

A handler is an `async fn` taking exactly one `Context` and returning anything
that implements `IntoResponse`:

```rust
#[route(GET "/text")]
async fn text(_ctx: Context) -> &'static str {
    "plain text"
}

#[route(GET "/json")]
async fn json(_ctx: Context) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

#[route(GET "/teapot")]
async fn teapot(_ctx: Context) -> Result<Response, Error> {
    Err(Error::new(StatusCode::IM_A_TEAPOT, "I'm a teapot"))
}
```

`Response`, `&str`, `String`, `Bytes`, `Vec<u8>`, `StatusCode`, `()`, `Json<T>`,
`Text<T>`, `Html<T>`, `Error` and `Result<T, E>` all implement `IntoResponse`.
An `Error` is rendered as `{"error": "..."}` with the matching status code.

## Reading a request

| Method                     | Description                                    |
| -------------------------- | ---------------------------------------------- |
| `ctx.method()`             | Request method                                 |
| `ctx.path()`               | Request path                                   |
| `ctx.uri()`                | Full URI, including the query string           |
| `ctx.param("id")`          | A path parameter                               |
| `ctx.params()`             | All path parameters                            |
| `ctx.query("q")`           | A query string value                           |
| `ctx.queries()`            | All query string values                        |
| `ctx.header("x")`          | A request header                               |
| `ctx.headers()`            | All request headers                            |
| `ctx.body()`               | The raw body                                   |
| `ctx.text()`               | The body as UTF-8                              |
| `ctx.json::<T>()`          | Deserialize the body as JSON                   |

### Typed request bodies

Declare `Context<T>` to have the JSON body deserialized for you. The value is
available as `ctx.req`, and a malformed body produces a `400` before the
handler runs:

```rust
#[derive(Deserialize, Schema)]
struct CreateTodoRequest {
    title: String,
    description: String,
}

#[route(POST "/todos")]
async fn create(ctx: Context<CreateTodoRequest>) -> Response {
    ctx.Json(serde_json::json!({ "title": ctx.req.title }))
}
```

## Building a response

`Context` offers `ctx.Json(value)`, `ctx.Text(text)` and `ctx.Html(markup)`,
which mirror `Response::json`, `Response::text` and `Response::html`.
Responses can be tweaked with `with_status` and `with_header`:

```rust
Response::json(serde_json::json!({ "id": 7 }))
    .with_status(StatusCode::CREATED)
    .with_header("x-request-id", "abc")
```

`#[derive(Response)]` builds a response type for you: it generates a builder,
an `IntoResponse` implementation and the OpenAPI response metadata.

```rust
#[derive(Serialize, Response)]
#[response(status = 201, description = "Todo created")]
struct CreateTodoResponse {
    title: String,
    description: String,
}

CreateTodoResponse::builder()
    .title("Ship it")
    .description("soon")
    .build()
    .into_response();
```

## OpenAPI

Annotate a route with `#[openapi(...)]` to describe it, then point clients at
`/openapi.json` or open `/docs` in a browser. The document is generated from the
registered routes, so it always matches what the server actually serves, and
`/docs` renders it with the [Scalar](https://github.com/scalar/scalar) API
reference.

```rust
#[route(POST "/todos")]
#[openapi(
    summary = "Create a new todo",
    description = "Create a new todo ...",
    tag = "Todo",
    request = CreateTodoRequest,
    responses = (
        CreateTodoResponse,
        ValidationErrorResponse,
        InternalErrorResponse
    )
)]
async fn create(ctx: Context<CreateTodoRequest>) -> Response {
    CreateTodoResponse::builder()
        .title(ctx.req.title)
        .description(ctx.req.description)
        .status(ctx.req.status)
        .build()
        .into_response()
}
```

The attribute supports `summary`, `description`, `tag` (repeatable), `tags`,
`operation_id`, `deprecated`, `request` and `responses`. Request bodies use
`#[derive(Schema)]`; response bodies use `#[derive(Response)]`. Built-in error
responses include `ValidationErrorResponse` (`400`), `NotFoundErrorResponse`
(`404`) and `InternalErrorResponse` (`500`).

> Deriving `Serialize`/`Deserialize` resolves the `serde` crate by name, so add
> `serde = { version = "1", features = ["derive"] }` to your `Cargo.toml` even
> though Lumia re-exports the traits.

Configure the document with `Server::openapi`, move it with
`Server::openapi_path`, or turn it off with `Server::disable_openapi`. The
Scalar reference moves with `Server::docs_path` and can be turned off on its own
with `Server::disable_docs`:

```rust
Server::new()
    .openapi(OpenApi::new("Todo API", "0.1.0"))
    .route(create);
```

## Serving

`Server::run(addr)` binds and serves forever. For tests, or to shut down
gracefully, bind a `tokio::net::TcpListener` yourself and use
`Server::serve(listener)` or `Server::serve_until(listener, signal)`.

```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
let addr = listener.local_addr()?;
tokio::spawn(server.serve(listener));
```

## Crates

| Crate             | Purpose                                            |
| ----------------- | -------------------------------------------------- |
| `lumia`           | Public façade: prelude, `Server`, `#[route]`, derives. |
| `lumia-core`      | Runtime: `Server`, `Router`, `Context`, `Response`. |
| `lumia-macros`    | The `#[route]`, `#[openapi]`, `Schema` and `Response` macros. |
| `lumia-openapi`   | OpenAPI 3.0 document generation.                    |
| `lumia-telemetry` | Tracing and metrics (planned).                      |

The OpenAPI example lives in [`examples/openapi`](../examples/openapi).
