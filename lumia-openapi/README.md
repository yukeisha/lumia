# lumia-openapi

OpenAPI 3.0 document generation for [Lumia](../lumia).

Routes annotated with `#[openapi(...)]` produce an `Operation`, which the
server turns into a document served at `/openapi.json`. This crate provides the
supporting data model:

* `OpenApiSchema` — describes a type as a JSON Schema (`#[derive(Schema)]`).
* `ApiResponse` — describes a response (`#[derive(Response)]`), with built-in
  `ValidationErrorResponse` (`400`), `NotFoundErrorResponse` (`404`) and
  `InternalErrorResponse` (`500`).
* `Operation`, `RequestBody`, `ResponseBody` — the collected metadata.
* `OpenApi` — the document builder.
* `scalar_html` — the HTML page that renders the document with Scalar.

```rust
use lumia::openapi::OpenApi;

let operations = server.router().operations();
let document = OpenApi::new("Todo API", "0.1.0").to_json(&operations);
```
