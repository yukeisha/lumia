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
