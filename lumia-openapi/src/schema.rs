//! JSON Schema descriptions for request and response types.

use serde_json::{Map, Value, json};

/// A JSON Schema together with the component name it is registered under.
///
/// The document builder turns a named schema into a `$ref` and registers the
/// schema under `components/schemas`, while an unnamed schema is inlined.
#[derive(Debug, Clone)]
pub struct Schema {
    /// The component name, or `None` when the schema is inlined.
    pub name: Option<String>,
    /// The inline JSON Schema.
    pub schema: Value,
}

impl Schema {
    /// Creates a named schema that becomes a `components/schemas` entry.
    pub fn named(name: impl Into<String>, schema: Value) -> Self {
        Self {
            name: Some(name.into()),
            schema,
        }
    }

    /// Creates an unnamed schema that is inlined where it is used.
    pub fn inline(schema: Value) -> Self {
        Self { name: None, schema }
    }
}

/// Types that can describe themselves as a JSON Schema.
///
/// Implement it with `#[derive(Schema)]` for application types, or rely on the
/// built-in implementations for primitives, `Option<T>`, `Vec<T>` and
/// `serde_json::Value`.
pub trait OpenApiSchema {
    /// The inline JSON Schema for this type.
    fn schema() -> Value;

    /// The name this type is registered under in `components/schemas`.
    ///
    /// Returns `None` for types that are always inlined, such as primitives.
    fn schema_name() -> Option<String> {
        None
    }

    /// The schema and, when available, its component name.
    fn component() -> Schema {
        Schema {
            name: Self::schema_name(),
            schema: Self::schema(),
        }
    }
}

fn nullable(mut schema: Value) -> Value {
    match &mut schema {
        Value::Object(map) => {
            map.insert("nullable".to_owned(), Value::Bool(true));
            schema
        }
        other => json!({ "nullable": true, "allOf": [other] }),
    }
}

macro_rules! impl_primitive {
    ($($ty:ty => $schema:expr),* $(,)?) => {
        $(
            impl OpenApiSchema for $ty {
                fn schema() -> Value {
                    $schema
                }
            }
        )*
    };
}

impl_primitive! {
    String => json!({ "type": "string" }),
    bool => json!({ "type": "boolean" }),
    i8 => json!({ "type": "integer", "format": "int32" }),
    i16 => json!({ "type": "integer", "format": "int32" }),
    i32 => json!({ "type": "integer", "format": "int32" }),
    i64 => json!({ "type": "integer", "format": "int64" }),
    isize => json!({ "type": "integer" }),
    u8 => json!({ "type": "integer", "minimum": 0 }),
    u16 => json!({ "type": "integer", "minimum": 0 }),
    u32 => json!({ "type": "integer", "minimum": 0 }),
    u64 => json!({ "type": "integer", "minimum": 0 }),
    usize => json!({ "type": "integer", "minimum": 0 }),
    f32 => json!({ "type": "number", "format": "float" }),
    f64 => json!({ "type": "number", "format": "double" }),
}

impl OpenApiSchema for serde_json::Value {
    fn schema() -> Value {
        json!({})
    }
}

impl<T: OpenApiSchema> OpenApiSchema for Option<T> {
    fn schema() -> Value {
        nullable(T::schema())
    }
}

impl<T: OpenApiSchema> OpenApiSchema for Vec<T> {
    fn schema() -> Value {
        json!({ "type": "array", "items": T::schema() })
    }
}

impl<T: OpenApiSchema> OpenApiSchema for Box<T> {
    fn schema() -> Value {
        T::schema()
    }
}

/// Builds a JSON object schema from already computed property schemas.
///
/// Used by the `Schema` and `Response` derives.
pub fn object_schema(title: &str, properties: Map<String, Value>, required: Vec<String>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    schema.insert("title".to_owned(), Value::String(title.to_owned()));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert(
            "required".to_owned(),
            Value::Array(required.into_iter().map(Value::String).collect()),
        );
    }
    Value::Object(schema)
}

/// Builds a string enum schema.
///
/// Used by the `Schema` derive.
pub fn enum_schema(title: &str, values: Vec<String>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_owned(), Value::String("string".to_owned()));
    schema.insert("title".to_owned(), Value::String(title.to_owned()));
    schema.insert(
        "enum".to_owned(),
        Value::Array(values.into_iter().map(Value::String).collect()),
    );
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_are_inline() {
        assert!(<String as OpenApiSchema>::schema_name().is_none());
        assert_eq!(
            <String as OpenApiSchema>::schema(),
            json!({ "type": "string" })
        );
    }

    #[test]
    fn options_are_nullable() {
        assert_eq!(
            <Option<String> as OpenApiSchema>::schema(),
            json!({ "type": "string", "nullable": true })
        );
    }

    #[test]
    fn vectors_become_arrays() {
        assert_eq!(
            <Vec<i32> as OpenApiSchema>::schema(),
            json!({ "type": "array", "items": { "type": "integer", "format": "int32" } })
        );
    }
}
