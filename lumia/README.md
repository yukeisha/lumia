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

## Building a response

`Context` offers `ctx.Json(value)`, `ctx.Text(text)` and `ctx.Html(markup)`,
which mirror `Response::json`, `Response::text` and `Response::html`.
Responses can be tweaked with `with_status` and `with_header`:

```rust
Response::json(serde_json::json!({ "id": 7 }))
    .with_status(StatusCode::CREATED)
    .with_header("x-request-id", "abc")
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
| `lumia`           | Public façade: prelude, `Server`, `#[route]`.       |
| `lumia-core`      | Runtime: `Server`, `Router`, `Context`, `Response`. |
| `lumia-macros`    | The `#[route]` attribute macro.                     |
| `lumia-openapi`   | OpenAPI generation (planned).                       |
| `lumia-telemetry` | Tracing and metrics (planned).                      |
