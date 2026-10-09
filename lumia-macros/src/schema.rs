//! `#[derive(Schema)]` and the schema reflection shared with `#[derive(Response)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, Error, Expr, ExprLit, Field, Fields, Lit,
    LitStr, Meta, Token, Type, Variant,
};

/// Expands `impl OpenApiSchema` for the input type.
pub fn schema_impl(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();
    let rename_all = container_rename_all(&input.attrs);

    let body = match &input.data {
        Data::Struct(data) => struct_schema(data, &name_str, &rename_all)?,
        Data::Enum(data) => enum_schema(data, &name_str, &rename_all)?,
        Data::Union(_) => {
            return Err(Error::new_spanned(
                name,
                "Schema cannot be derived for unions",
            ));
        }
    };

    Ok(quote! {
        impl ::lumia::openapi::OpenApiSchema for #name {
            fn schema() -> ::lumia::serde_json::Value {
                #body
            }

            fn schema_name() -> ::std::option::Option<::std::string::String> {
                ::std::option::Option::Some(::std::string::String::from(#name_str))
            }
        }
    })
}

fn struct_schema(
    data: &DataStruct,
    name: &str,
    rename_all: &Option<String>,
) -> syn::Result<TokenStream> {
    let fields: Vec<&Field> = match &data.fields {
        Fields::Named(named) => named.named.iter().collect(),
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => {
            return Err(Error::new_spanned(
                unnamed,
                "Schema can only be derived for structs with named fields",
            ));
        }
    };

    let mut inserts = Vec::new();
    let mut required = Vec::new();

    for field in fields {
        if is_skipped(&field.attrs) {
            continue;
        }
        let ident = field
            .ident
            .as_ref()
            .expect("named fields always have an identifier");
        let property = field_name(ident, &field.attrs, rename_all);
        let ty = &field.ty;
        inserts.push(quote! {
            __lumia_properties.insert(
                ::std::string::String::from(#property),
                <#ty as ::lumia::openapi::OpenApiSchema>::schema(),
            );
        });
        if !is_option(ty) {
            required.push(property);
        }
    }

    Ok(quote! {
        {
            let mut __lumia_properties = ::lumia::serde_json::Map::new();
            #(#inserts)*
            ::lumia::openapi::object_schema(
                #name,
                __lumia_properties,
                ::std::vec![ #( ::std::string::String::from(#required) ),* ],
            )
        }
    })
}

fn enum_schema(
    data: &DataEnum,
    name: &str,
    rename_all: &Option<String>,
) -> syn::Result<TokenStream> {
    let mut values = Vec::new();

    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(Error::new_spanned(
                variant,
                "Schema can only be derived for enums with unit variants",
            ));
        }
        if is_skipped(&variant.attrs) {
            continue;
        }
        values.push(variant_name(variant, rename_all));
    }

    Ok(quote! {
        ::lumia::openapi::enum_schema(
            #name,
            ::std::vec![ #( ::std::string::String::from(#values) ),* ],
        )
    })
}

/// The JSON property name for a struct field.
pub fn field_name(ident: &syn::Ident, attrs: &[Attribute], rename_all: &Option<String>) -> String {
    let raw = strip_raw(ident);
    if let Some(rename) = rename_from(attrs) {
        return rename;
    }
    match rename_all {
        Some(rule) => apply_rename(rule, &raw),
        None => raw,
    }
}

fn variant_name(variant: &Variant, rename_all: &Option<String>) -> String {
    let raw = strip_raw(&variant.ident);
    if let Some(rename) = rename_from(&variant.attrs) {
        return rename;
    }
    match rename_all {
        Some(rule) => apply_rename(rule, &raw),
        None => raw,
    }
}

fn strip_raw(ident: &syn::Ident) -> String {
    ident
        .to_string()
        .strip_prefix("r#")
        .unwrap_or(&ident.to_string())
        .to_owned()
}

/// Whether a field is skipped via `#[serde(skip)]` or `#[schema(skip)]`.
pub fn is_skipped(attrs: &[Attribute]) -> bool {
    metas(attrs)
        .iter()
        .any(|meta| matches!(meta, Meta::Path(path) if path.is_ident("skip")))
}

/// Whether a field type is syntactically `Option<...>`.
pub fn is_option(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Option")
}

fn container_rename_all(attrs: &[Attribute]) -> Option<String> {
    metas(attrs).iter().find_map(|meta| match meta {
        Meta::NameValue(name_value) if name_value.path.is_ident("rename_all") => {
            literal_string(&name_value.value)
        }
        Meta::List(list) if list.path.is_ident("rename_all") => {
            list.parse_args::<LitStr>().ok().map(|lit| lit.value())
        }
        _ => None,
    })
}

fn rename_from(attrs: &[Attribute]) -> Option<String> {
    metas(attrs).iter().find_map(|meta| match meta {
        Meta::NameValue(name_value) if name_value.path.is_ident("rename") => {
            literal_string(&name_value.value)
        }
        _ => None,
    })
}

fn metas(attrs: &[Attribute]) -> Vec<Meta> {
    let mut collected = Vec::new();
    for attr in attrs {
        if !(attr.path().is_ident("serde") || attr.path().is_ident("schema")) {
            continue;
        }
        if let Meta::List(list) = &attr.meta
            && let Ok(items) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        {
            collected.extend(items);
        }
    }
    collected
}

fn literal_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Some(value.value()),
        _ => None,
    }
}

fn apply_rename(rule: &str, name: &str) -> String {
    let words = split_words(name);
    match rule {
        "lowercase" => words.concat().to_lowercase(),
        "UPPERCASE" => words.concat().to_uppercase(),
        "snake_case" => words
            .iter()
            .map(|word| word.to_lowercase())
            .collect::<Vec<_>>()
            .join("_"),
        "SCREAMING_SNAKE_CASE" => words
            .iter()
            .map(|word| word.to_uppercase())
            .collect::<Vec<_>>()
            .join("_"),
        "kebab-case" => words
            .iter()
            .map(|word| word.to_lowercase())
            .collect::<Vec<_>>()
            .join("-"),
        "SCREAMING-KEBAB-CASE" => words
            .iter()
            .map(|word| word.to_uppercase())
            .collect::<Vec<_>>()
            .join("-"),
        "camelCase" => {
            let mut iter = words.iter();
            match iter.next() {
                Some(first) => {
                    let mut result = first.to_lowercase();
                    for word in iter {
                        result.push_str(&capitalize(word));
                    }
                    result
                }
                None => String::new(),
            }
        }
        "PascalCase" => words.iter().map(|word| capitalize(word)).collect(),
        _ => name.to_owned(),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn split_words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().filter(|c| *c != '_' && *c != '-').collect();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, value) in chars.iter().enumerate() {
        let uppercase = value.is_ascii_uppercase();
        let starts_word = uppercase
            && !current.is_empty()
            && (chars[index - 1].is_ascii_lowercase()
                || chars[index - 1].is_ascii_digit()
                || chars
                    .get(index + 1)
                    .is_some_and(|next| next.is_ascii_lowercase()));
        if starts_word {
            words.push(std::mem::take(&mut current));
        }
        current.push(*value);
    }

    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_follow_serde_rules() {
        assert_eq!(apply_rename("snake_case", "InProgress"), "in_progress");
        assert_eq!(
            apply_rename("SCREAMING_SNAKE_CASE", "InProgress"),
            "IN_PROGRESS"
        );
        assert_eq!(apply_rename("camelCase", "InProgress"), "inProgress");
        assert_eq!(apply_rename("kebab-case", "InProgress"), "in-progress");
        assert_eq!(apply_rename("lowercase", "InProgress"), "inprogress");
    }

    #[test]
    fn detects_option_types() {
        assert!(is_option(&syn::parse_quote!(Option<String>)));
        assert!(!is_option(&syn::parse_quote!(String)));
        assert!(!is_option(&syn::parse_quote!(Vec<String>)));
    }

    #[test]
    fn reads_serde_rename_and_skip() {
        let field: Field = syn::parse_quote! {
            #[serde(rename = "userId", skip)]
            id: String
        };
        assert!(is_skipped(&field.attrs));
        assert_eq!(
            rename_from(&field.attrs).as_deref(),
            Some("userId"),
            "rename should be read"
        );
    }

    #[test]
    fn reads_container_rename_all() {
        use syn::parse::Parser;

        let attrs = syn::Attribute::parse_outer
            .parse2(quote!(#[serde(rename_all = "snake_case")]))
            .unwrap();
        assert_eq!(container_rename_all(&attrs).as_deref(), Some("snake_case"));
    }
}
