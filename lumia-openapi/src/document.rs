//! Builds an OpenAPI 3.0 document from collected [`Operation`]s.

use serde_json::{Map, Value, json};

use crate::operation::{Operation, RequestBody, ResponseBody};
use crate::schema::Schema;

/// The document metadata and builder.
#[derive(Debug, Clone)]
pub struct OpenApi {
    /// The API title.
    pub title: String,
    /// The API version.
    pub version: String,
    /// An optional API description.
    pub description: Option<String>,
}

impl OpenApi {
    /// Creates a document with the given title and version.
    pub fn new(title: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            version: version.into(),
            description: None,
        }
    }

    /// Sets the API description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Builds the OpenAPI document as JSON.
    pub fn document<'a>(&self, operations: impl IntoIterator<Item = &'a Operation>) -> Value {
        let mut paths: Map<String, Value> = Map::new();
        let mut components: Map<String, Value> = Map::new();

        for operation in operations {
            let entry = build_operation(operation, &mut components);
            let item = paths
                .entry(operation.path.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(item) = item {
                item.insert(operation.method.to_lowercase(), entry);
            }
        }

        let mut info = Map::new();
        info.insert("title".to_owned(), Value::String(self.title.clone()));
        info.insert("version".to_owned(), Value::String(self.version.clone()));
        if let Some(description) = &self.description {
            info.insert("description".to_owned(), Value::String(description.clone()));
        }

        let mut components_root = Map::new();
        components_root.insert("schemas".to_owned(), Value::Object(components));

        json!({
            "openapi": "3.0.3",
            "info": Value::Object(info),
            "paths": Value::Object(paths),
            "components": Value::Object(components_root),
        })
    }

    /// Builds the document and serializes it to a JSON string.
    pub fn to_json<'a>(&self, operations: impl IntoIterator<Item = &'a Operation>) -> String {
        serde_json::to_string_pretty(&self.document(operations)).unwrap_or_else(|_| "{}".to_owned())
    }
}

fn build_operation(operation: &Operation, components: &mut Map<String, Value>) -> Value {
    let mut object = Map::new();

    object.insert(
        "operationId".to_owned(),
        Value::String(
            operation
                .operation_id
                .clone()
                .unwrap_or_else(|| default_operation_id(operation)),
        ),
    );
    if let Some(summary) = &operation.summary {
        object.insert("summary".to_owned(), Value::String(summary.clone()));
    }
    if let Some(description) = &operation.description {
        object.insert("description".to_owned(), Value::String(description.clone()));
    }
    if !operation.tags.is_empty() {
        object.insert(
            "tags".to_owned(),
            Value::Array(
                operation
                    .tags
                    .iter()
                    .map(|tag| Value::String(tag.clone()))
                    .collect(),
            ),
        );
    }
    if operation.deprecated {
        object.insert("deprecated".to_owned(), Value::Bool(true));
    }
    if let Some(request) = &operation.request {
        object.insert(
            "requestBody".to_owned(),
            build_request_body(request, components),
        );
    }
    object.insert(
        "responses".to_owned(),
        build_responses(&operation.responses, components),
    );

    Value::Object(object)
}

fn default_operation_id(operation: &Operation) -> String {
    let path = operation
        .path
        .trim_matches('/')
        .replace(['{', '}'], "")
        .replace('/', "_");
    let path = if path.is_empty() {
        "root".to_owned()
    } else {
        path
    };
    format!("{}_{}", operation.method.to_lowercase(), path)
}

fn build_request_body(request: &RequestBody, components: &mut Map<String, Value>) -> Value {
    let schema = schema_value(&request.schema, components);
    let mut content = Map::new();
    content.insert(request.content_type.clone(), json!({ "schema": schema }));
    json!({
        "required": request.required,
        "content": Value::Object(content),
    })
}

fn build_responses(responses: &[ResponseBody], components: &mut Map<String, Value>) -> Value {
    let mut object = Map::new();

    if responses.is_empty() {
        object.insert("200".to_owned(), json!({ "description": "OK" }));
        return Value::Object(object);
    }

    for response in responses {
        let mut entry = Map::new();
        entry.insert(
            "description".to_owned(),
            Value::String(response.description.clone()),
        );
        if let Some(schema) = &response.schema {
            let schema = schema_value(schema, components);
            entry.insert(
                "content".to_owned(),
                json!({ "application/json": { "schema": schema } }),
            );
        }
        object.insert(response.status.to_string(), Value::Object(entry));
    }

    Value::Object(object)
}

fn schema_value(schema: &Schema, components: &mut Map<String, Value>) -> Value {
    match &schema.name {
        Some(name) => {
            components.insert(name.clone(), schema.schema.clone());
            json!({ "$ref": format!("#/components/schemas/{name}") })
        }
        None => schema.schema.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::Operation;
    use crate::response::ValidationErrorResponse;

    fn operation() -> Operation {
        Operation {
            method: "POST".to_owned(),
            path: "/todos".to_owned(),
            summary: Some("Create a new todo".to_owned()),
            description: None,
            operation_id: None,
            tags: vec!["Todo".to_owned()],
            deprecated: false,
            request: Some(RequestBody {
                content_type: "application/json".to_owned(),
                required: true,
                schema: Schema::named(
                    "CreateTodoRequest",
                    json!({ "type": "object", "properties": {} }),
                ),
            }),
            responses: vec![ResponseBody::of::<ValidationErrorResponse>()],
        }
    }

    #[test]
    fn builds_paths_and_components() {
        let document = OpenApi::new("Todo API", "1.0.0").document([&operation()]);

        assert_eq!(document["openapi"], "3.0.3");
        assert_eq!(document["info"]["title"], "Todo API");
        assert_eq!(
            document["paths"]["/todos"]["post"]["summary"],
            "Create a new todo"
        );
        assert_eq!(
            document["paths"]["/todos"]["post"]["requestBody"]["content"]["application/json"]["schema"]
                ["$ref"],
            "#/components/schemas/CreateTodoRequest"
        );
        assert_eq!(
            document["paths"]["/todos"]["post"]["responses"]["400"]["description"],
            "Validation error"
        );
        assert!(document["components"]["schemas"]["CreateTodoRequest"].is_object());
    }

    #[test]
    fn defaults_to_ok_response() {
        let mut operation = operation();
        operation.responses.clear();
        let document = OpenApi::new("t", "v").document([&operation]);
        assert_eq!(
            document["paths"]["/todos"]["post"]["responses"]["200"]["description"],
            "OK"
        );
    }
}
