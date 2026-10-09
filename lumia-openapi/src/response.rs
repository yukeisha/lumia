//! Response descriptions used by OpenAPI operations.

use crate::schema::Schema;

/// A response type that can be listed in an operation.
///
/// Implement it with `#[derive(Response)]`, or use one of the built-in error
/// responses such as [`ValidationErrorResponse`] and [`InternalErrorResponse`].
pub trait ApiResponse {
    /// The HTTP status code of the response.
    fn status_code() -> u16;

    /// The human readable description of the response.
    fn description() -> &'static str;

    /// The JSON schema of the response body, or `None` for an empty body.
    fn schema() -> Option<Schema>;
}

fn error_schema(name: &str) -> Schema {
    Schema::named(
        name,
        crate::schema::object_schema(
            name,
            [("error".to_owned(), serde_json::json!({ "type": "string" }))]
                .into_iter()
                .collect(),
            vec!["error".to_owned()],
        ),
    )
}

/// Built-in `400 Bad Request` response for request validation failures.
#[derive(Debug, Clone, Copy)]
pub struct ValidationErrorResponse;

impl ApiResponse for ValidationErrorResponse {
    fn status_code() -> u16 {
        400
    }

    fn description() -> &'static str {
        "Validation error"
    }

    fn schema() -> Option<Schema> {
        Some(error_schema("ValidationError"))
    }
}

/// Built-in `500 Internal Server Error` response.
#[derive(Debug, Clone, Copy)]
pub struct InternalErrorResponse;

impl ApiResponse for InternalErrorResponse {
    fn status_code() -> u16 {
        500
    }

    fn description() -> &'static str {
        "Internal server error"
    }

    fn schema() -> Option<Schema> {
        Some(error_schema("InternalError"))
    }
}

/// Built-in `404 Not Found` response.
#[derive(Debug, Clone, Copy)]
pub struct NotFoundErrorResponse;

impl ApiResponse for NotFoundErrorResponse {
    fn status_code() -> u16 {
        404
    }

    fn description() -> &'static str {
        "Not found"
    }

    fn schema() -> Option<Schema> {
        Some(error_schema("NotFoundError"))
    }
}
