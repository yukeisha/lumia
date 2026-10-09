//! Parsing for the `#[openapi(...)]` metadata attached to routes.

use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, LitBool, LitStr, Token, Type};

/// OpenAPI metadata collected from `#[openapi(...)]` and the trailing keys of
/// `#[route(...)]`.
#[derive(Default, Clone)]
pub struct OpenApiMeta {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub operation_id: Option<String>,
    pub deprecated: bool,
    pub request: Option<Type>,
    pub responses: Vec<Type>,
}

impl OpenApiMeta {
    /// Whether any metadata was provided at all.
    pub fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.description.is_none()
            && self.tags.is_empty()
            && self.operation_id.is_none()
            && !self.deprecated
            && self.request.is_none()
            && self.responses.is_empty()
    }

    /// Merges `other` into `self`, with `other` taking precedence.
    pub fn merge(&mut self, other: OpenApiMeta) {
        self.summary = other.summary.or_else(|| self.summary.take());
        self.description = other.description.or_else(|| self.description.take());
        self.tags.extend(other.tags);
        self.operation_id = other.operation_id.or_else(|| self.operation_id.take());
        self.deprecated |= other.deprecated;
        self.request = other.request.or_else(|| self.request.take());
        self.responses.extend(other.responses);
    }

    /// Parses a single `key = value` entry.
    pub fn parse_entry(&mut self, input: ParseStream) -> syn::Result<()> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "summary" => self.summary = Some(input.parse::<LitStr>()?.value()),
            "description" => self.description = Some(input.parse::<LitStr>()?.value()),
            "tag" => self.tags.push(input.parse::<LitStr>()?.value()),
            "tags" => self.tags.extend(parse_str_list(input)?),
            "operation_id" => self.operation_id = Some(input.parse::<LitStr>()?.value()),
            "deprecated" => self.deprecated = input.parse::<LitBool>()?.value(),
            "request" => self.request = Some(input.parse::<Type>()?),
            "responses" => self.responses = parse_type_list(input)?,
            other => {
                return Err(Error::new(
                    key.span(),
                    format!(
                        "unknown openapi key `{other}`, expected one of: summary, description, \
                         tag, tags, operation_id, deprecated, request, responses"
                    ),
                ));
            }
        }
        Ok(())
    }
}

/// The body of an `#[openapi(...)]` attribute.
pub struct OpenApiMetaBody(pub OpenApiMeta);

impl Parse for OpenApiMetaBody {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut meta = OpenApiMeta::default();
        parse_entries(input, &mut meta)?;
        Ok(Self(meta))
    }
}

/// Parses a comma separated list of `key = value` entries into `meta`.
pub fn parse_entries(input: ParseStream, meta: &mut OpenApiMeta) -> syn::Result<()> {
    while !input.is_empty() {
        meta.parse_entry(input)?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if !input.is_empty() {
            return Err(input.error("expected `,` between openapi entries"));
        }
    }
    Ok(())
}

fn parse_str_list(input: ParseStream) -> syn::Result<Vec<String>> {
    let content;
    if input.peek(syn::token::Bracket) {
        syn::bracketed!(content in input);
    } else if input.peek(syn::token::Paren) {
        syn::parenthesized!(content in input);
    } else {
        return Err(input.error("expected a bracketed list of strings"));
    }

    let mut values = Vec::new();
    while !content.is_empty() {
        values.push(content.parse::<LitStr>()?.value());
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else {
            break;
        }
    }
    Ok(values)
}

fn parse_type_list(input: ParseStream) -> syn::Result<Vec<Type>> {
    let content;
    if input.peek(syn::token::Paren) {
        syn::parenthesized!(content in input);
    } else if input.peek(syn::token::Bracket) {
        syn::bracketed!(content in input);
    } else {
        return Err(input.error("expected a parenthesized list of response types"));
    }

    let mut types = Vec::new();
    while !content.is_empty() {
        types.push(content.parse::<Type>()?);
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        } else {
            break;
        }
    }
    Ok(types)
}
