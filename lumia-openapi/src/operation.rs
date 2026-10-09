//! Operation metadata collected from routes.

use crate::response::ApiResponse;
use crate::schema::{OpenApiSchema, Schema};

/// The request body of an operation.
#[derive(Debug, Clone)]
pub struct RequestBody {
    /// The content type of the body.
    pub content_type: String,
    /// Whether the body is required.
    pub required: bool,
    /// The body schema.
    pub schema: Schema,
}

impl RequestBody {
    /// Describes an `application/json` request body of type `T`.
    pub fn json<T: OpenApiSchema>() -> Self {
        Self {
            content_type: "application/json".to_owned(),
            required: true,
            schema: T::component(),
        }
    }
}

/// A single response of an operation.
#[derive(Debug, Clone)]
pub struct ResponseBody {
    /// The HTTP status code.
    pub status: u16,
    /// The human readable description.
    pub description: String,
    /// The response body schema, or `None` for an empty body.
    pub schema: Option<Schema>,
}

impl ResponseBody {
    /// Describes a response from an [`ApiResponse`] implementation.
    pub fn of<T: ApiResponse>() -> Self {
        Self {
            status: T::status_code(),
            description: T::description().to_owned(),
            schema: T::schema(),
        }
    }
}

/// Everything the OpenAPI document needs to describe a single route handler.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The HTTP method, for example `POST`.
    pub method: String,
    /// The path template, for example `/todos`.
    pub path: String,
    /// A short summary.
    pub summary: Option<String>,
    /// A longer description.
    pub description: Option<String>,
    /// A stable operation id.
    pub operation_id: Option<String>,
    /// The tags the operation belongs to.
    pub tags: Vec<String>,
    /// Whether the operation is deprecated.
    pub deprecated: bool,
    /// The request body, when the operation accepts one.
    pub request: Option<RequestBody>,
    /// The declared responses.
    pub responses: Vec<ResponseBody>,
}
