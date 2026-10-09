//! Attribute and derive macros for Lumia.
//!
//! The main entry point is [`route`], which turns an async function into a
//! route that can be passed to `Server::route`. An optional `openapi` attribute
//! describes the route for the generated OpenAPI document, and
//! [`Schema`]/[`Response`] describe request and response bodies.

mod metadata;
mod response;
mod schema;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Error, FnArg, GenericArgument, Ident, ItemFn, LitStr, Meta, PathArguments, Token,
    Type, parse_macro_input,
};

use metadata::{OpenApiMeta, OpenApiMetaBody};

/// Declares an HTTP route.
///
/// The macro keeps the function body but renames it, and exposes the original
/// name as a value implementing `lumia::Route`. That value carries the HTTP
/// method and path, so it can be registered directly:
///
/// ```text
/// use lumia::prelude::*;
///
/// #[tokio::main]
/// async fn main() {
///     let mut server = Server::new();
///     server.route(greet);
///     server.run("0.0.0.0:3000").await.unwrap();
/// }
///
/// #[route(GET "/")]
/// async fn greet(ctx: Context) -> Response {
///     ctx.Json(serde_json::json!({ "message": "Hello, World!" }))
/// }
/// ```
///
/// The method defaults to `GET` when it is omitted:
///
/// ```text
/// #[route("/health")]
/// async fn health(ctx: Context) -> Response {
///     Response::text("ok")
/// }
/// ```
///
/// Paths support `{name}` (or `:name`) parameters and a trailing `*name`
/// catch-all segment, both readable through `Context::param`.
///
/// A handler may declare a typed body with `Context<T>`, in which case the
/// body is deserialized as JSON before the handler runs and made available as
/// `ctx.req`:
///
/// ```text
/// #[route(POST "/todos")]
/// async fn create(ctx: Context<CreateTodoRequest>) -> Response {
///     let title = ctx.req.title;
///     // ...
/// }
/// ```
///
/// The handler must be an `async fn` taking exactly one argument (a
/// `Context`) and no generics.
///
/// # Note
///
/// The macro keeps the function under a hidden name and defines a unit struct
/// with the original name in the same scope. Rust therefore treats that name as
/// a unit struct in patterns, so avoid binding a local variable with the exact
/// name of a handler declared in the same module.
#[proc_macro_attribute]
pub fn route(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = match syn::parse::<RouteArgs>(args) {
        Ok(args) => args,
        Err(error) => return error.to_compile_error().into(),
    };

    let function = match syn::parse::<ItemFn>(item) {
        Ok(function) => function,
        Err(error) => return error.to_compile_error().into(),
    };

    expand(args, function)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Describes a route for the generated OpenAPI document.
///
/// Place it after `#[route(...)]` (the order used in the examples) or before
/// it; either way the metadata is attached to the generated route.
///
/// ```text
/// #[route(POST "/todos")]
/// #[openapi(
///     summary = "Create a new todo",
///     tag = "Todo",
///     request = CreateTodoRequest,
///     responses = (CreateTodoResponse, ValidationErrorResponse),
/// )]
/// async fn create(ctx: Context<CreateTodoRequest>) -> Response {
///     // ...
/// }
/// ```
///
/// Supported keys are `summary`, `description`, `tag` (repeatable), `tags`,
/// `operation_id`, `deprecated`, `request` and `responses`.
#[proc_macro_attribute]
pub fn openapi(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = TokenStream2::from(args);
    let item = TokenStream2::from(item);

    match merge_into_route(&args, item.clone()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derives an OpenAPI JSON schema for a struct or enum.
///
/// Structs become object schemas and enums with unit variants become string
/// enums. Field and variant names honour `#[serde(rename = "...")]` and
/// `#[serde(rename_all = "...")]`; skipped items honour `#[serde(skip)]`.
///
/// ```text
/// #[derive(Serialize, Schema)]
/// struct CreateTodoRequest {
///     title: String,
///     description: Option<String>,
/// }
/// ```
#[proc_macro_derive(Schema, attributes(schema, serde))]
pub fn derive_schema(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
    schema::schema_impl(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives a response type: a builder, `IntoResponse` and OpenAPI metadata.
///
/// ```text
/// #[derive(Serialize, Response)]
/// #[response(status = 201, description = "Todo created")]
/// struct CreateTodoResponse {
///     title: String,
///     description: String,
/// }
///
/// CreateTodoResponse::builder()
///     .title("Ship it")
///     .build()
///     .into_response();
/// ```
#[proc_macro_derive(Response, attributes(response, schema, serde))]
pub fn derive_response(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
    response::response_impl(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn merge_into_route(args: &TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let Ok(mut function) = syn::parse2::<ItemFn>(item.clone()) else {
        return Ok(item);
    };

    let Some(index) = function
        .attrs
        .iter()
        .position(|attr| attr.path().is_ident("route"))
    else {
        // Used without `#[route]`; nothing to merge into.
        return Ok(item);
    };

    let route_attr = function.attrs.remove(index);
    let Meta::List(list) = route_attr.meta else {
        return Err(Error::new_spanned(
            route_attr,
            "`#[route]` must be written as `#[route(METHOD \"/path\")]`",
        ));
    };
    let route_args = list.tokens;
    let combined = if route_args.is_empty() {
        args.clone()
    } else {
        quote!(#route_args, #args)
    };

    let attrs = &function.attrs;
    let vis = &function.vis;
    let signature = &function.sig;
    let block = &function.block;

    Ok(quote! {
        #[route(#combined)]
        #(#attrs)*
        #vis #signature #block
    })
}

fn expand(args: RouteArgs, function: ItemFn) -> syn::Result<TokenStream2> {
    let span = function.sig.ident.span();

    if function.sig.asyncness.is_none() {
        return Err(Error::new(span, "route handlers must be `async fn`"));
    }

    if function.sig.generics.params.iter().next().is_some()
        || function.sig.generics.where_clause.is_some()
    {
        return Err(Error::new(span, "route handlers cannot be generic"));
    }

    if function.sig.inputs.len() != 1 {
        return Err(Error::new(
            span,
            "route handlers must take exactly one argument: `ctx: Context`",
        ));
    }

    let mut function = function;
    let mut attrs = std::mem::take(&mut function.attrs);
    let attribute_meta = take_openapi_meta(&mut attrs)?;

    let mut meta = args.meta.clone();
    meta.merge(attribute_meta);

    let name = function.sig.ident.clone();
    let handler = Ident::new(&format!("__lumia_handler_{name}"), span);
    let method = args.method.variant();
    let method_name = args.method.name();
    let path = args.path;
    let mirrored = mirror_attrs(&attrs);
    let vis = &function.vis;
    let payload = context_payload(&function.sig.inputs);
    let mut signature = function.sig.clone();
    signature.ident = handler.clone();
    let body = &function.block;

    let call = match payload {
        Some(payload) => quote! {
            let __lumia_payload = match ctx.json::<#payload>() {
                ::std::result::Result::Ok(payload) => payload,
                ::std::result::Result::Err(error) => {
                    return ::lumia::IntoResponse::into_response(error);
                }
            };
            let ctx = ctx.map_req(__lumia_payload);
            ::lumia::IntoResponse::into_response(#handler(ctx).await)
        },
        None => quote! {
            ::lumia::IntoResponse::into_response(#handler(ctx).await)
        },
    };

    let operation = build_operation(&meta, method_name, &path);

    Ok(quote! {
        #(#mirrored)*
        #[allow(non_camel_case_types)]
        #vis struct #name;

        impl ::lumia::Route for #name {
            fn method(&self) -> ::lumia::Method {
                ::lumia::Method::#method
            }

            fn path(&self) -> &'static str {
                #path
            }

            fn call(
                &self,
                ctx: ::lumia::Context,
            ) -> ::lumia::BoxFuture<'static, ::lumia::Response> {
                ::std::boxed::Box::pin(async move {
                    #call
                })
            }

            #operation
        }

        #(#attrs)*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #signature #body
    })
}

fn build_operation(meta: &OpenApiMeta, method: &str, path: &LitStr) -> TokenStream2 {
    if meta.is_empty() {
        return TokenStream2::new();
    }

    let summary = optional_string(&meta.summary);
    let description = optional_string(&meta.description);
    let operation_id = optional_string(&meta.operation_id);
    let tags = &meta.tags;
    let deprecated = meta.deprecated;
    let request = match &meta.request {
        Some(ty) => quote! {
            ::std::option::Option::Some(::lumia::openapi::RequestBody::json::<#ty>())
        },
        None => quote! { ::std::option::Option::None },
    };
    let responses = &meta.responses;

    quote! {
        fn operation(&self) -> ::std::option::Option<::lumia::openapi::Operation> {
            ::std::option::Option::Some(::lumia::openapi::Operation {
                method: ::std::string::String::from(#method),
                path: ::std::string::String::from(#path),
                summary: #summary,
                description: #description,
                operation_id: #operation_id,
                tags: ::std::vec![
                    #( ::std::string::String::from(#tags) ),*
                ],
                deprecated: #deprecated,
                request: #request,
                responses: ::std::vec![
                    #( ::lumia::openapi::ResponseBody::of::<#responses>() ),*
                ],
            })
        }
    }
}

fn optional_string(value: &Option<String>) -> TokenStream2 {
    match value {
        Some(value) => quote! {
            ::std::option::Option::Some(::std::string::String::from(#value))
        },
        None => quote! { ::std::option::Option::None },
    }
}

/// The payload type of a `Context<T>` handler, or `None` for a plain context.
fn context_payload(inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>) -> Option<Type> {
    let FnArg::Typed(argument) = inputs.first()? else {
        return None;
    };
    let Type::Path(path) = &*argument.ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Context" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let GenericArgument::Type(payload) = arguments.args.first()? else {
        return None;
    };
    if matches!(payload, Type::Tuple(tuple) if tuple.elems.is_empty()) {
        return None;
    }
    Some(payload.clone())
}

/// Removes and parses `#[openapi(...)]` attributes from `attrs`.
fn take_openapi_meta(attrs: &mut Vec<Attribute>) -> syn::Result<OpenApiMeta> {
    let mut meta = OpenApiMeta::default();
    let mut kept = Vec::new();

    for attr in attrs.drain(..) {
        if attr.path().is_ident("openapi") {
            let Meta::List(list) = &attr.meta else {
                return Err(Error::new_spanned(&attr, "expected `#[openapi(...)]`"));
            };
            let body = syn::parse2::<OpenApiMetaBody>(list.tokens.clone())?;
            meta.merge(body.0);
        } else {
            kept.push(attr);
        }
    }

    *attrs = kept;
    Ok(meta)
}

/// Attributes that make sense on both the handler and the generated struct.
///
/// Other attributes (for example `#[tracing::instrument]`) are only meaningful
/// on the function, so they are not mirrored onto the struct.
fn mirror_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    const MIRRORED: &[&str] = &[
        "doc",
        "cfg",
        "cfg_attr",
        "allow",
        "warn",
        "deny",
        "forbid",
        "deprecated",
    ];

    attrs
        .iter()
        .filter(|attr| MIRRORED.iter().any(|name| attr.path().is_ident(name)))
        .cloned()
        .collect()
}

struct RouteArgs {
    method: Method,
    path: LitStr,
    meta: OpenApiMeta,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (method, path) = if input.peek(LitStr) {
            (Method::Get, input.parse::<LitStr>()?)
        } else {
            let ident: Ident = input.parse().map_err(|_| {
                input.error("expected an HTTP method and a path, for example `#[route(GET \"/\")]`")
            })?;
            let method = Method::from_ident(&ident)?;

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }

            let path = input.parse::<LitStr>().map_err(|_| {
                Error::new(
                    ident.span(),
                    "expected a path string literal, for example `#[route(GET \"/\")]`",
                )
            })?;
            (method, path)
        };

        let mut meta = OpenApiMeta::default();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            meta.parse_entry(input)?;
        }

        finish(input)?;

        Ok(Self { method, path, meta })
    }
}

fn finish(input: ParseStream) -> syn::Result<()> {
    if input.is_empty() {
        Ok(())
    } else {
        Err(input.error("unexpected tokens; expected `#[route(GET \"/path\")]`"))
    }
}

enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Trace,
    Connect,
}

impl Method {
    const NAMES: &'static [&'static str] = &[
        "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT",
    ];

    fn from_ident(ident: &Ident) -> syn::Result<Self> {
        match ident.to_string().to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            "TRACE" => Ok(Self::Trace),
            "CONNECT" => Ok(Self::Connect),
            other => Err(Error::new(
                ident.span(),
                format!(
                    "unsupported HTTP method `{other}`, expected one of: {}",
                    Self::NAMES.join(", ")
                ),
            )),
        }
    }

    fn variant(&self) -> Ident {
        Ident::new(self.name(), proc_macro2::Span::call_site())
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Connect => "CONNECT",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn mirror(input: TokenStream2) -> Vec<String> {
        use syn::parse::Parser;

        let attrs = syn::Attribute::parse_outer.parse2(input).unwrap();
        mirror_attrs(&attrs)
            .iter()
            .map(|attr| attr.path().get_ident().unwrap().to_string())
            .collect()
    }

    fn args(input: TokenStream2) -> RouteArgs {
        syn::parse2::<RouteArgs>(input).unwrap()
    }

    fn function(input: TokenStream2) -> ItemFn {
        syn::parse2::<ItemFn>(input).unwrap()
    }

    #[test]
    fn mirrors_item_agnostic_attributes() {
        let mirrored = mirror(quote! {
            /// Docs
            #[cfg(feature = "api")]
            #[allow(unused_variables)]
        });

        assert_eq!(mirrored, vec!["doc", "cfg", "allow"]);
    }

    #[test]
    fn skips_function_only_attributes() {
        let mirrored = mirror(quote! {
            #[tracing::instrument]
            #[some_other_macro(arg)]
        });

        assert!(mirrored.is_empty(), "unexpected: {mirrored:?}");
    }

    #[test]
    fn parses_method_and_path() {
        let parsed = args(quote!(POST "/users"));
        assert!(matches!(parsed.method, Method::Post));
        assert_eq!(parsed.path.value(), "/users");
    }

    #[test]
    fn parses_comma_separated_and_lowercase_forms() {
        let parsed = args(quote!(delete, "/users/{id}"));
        assert!(matches!(parsed.method, Method::Delete));
        assert_eq!(parsed.path.value(), "/users/{id}");
    }

    #[test]
    fn parses_inline_openapi_metadata() {
        let parsed = args(quote!(
            POST "/todos",
            summary = "Create a new todo",
            tag = "Todo",
            request = CreateTodoRequest,
            responses = (CreateTodoResponse,)
        ));

        assert_eq!(parsed.meta.summary.as_deref(), Some("Create a new todo"));
        assert_eq!(parsed.meta.tags, vec!["Todo".to_owned()]);
        assert!(parsed.meta.request.is_some());
        assert_eq!(parsed.meta.responses.len(), 1);
    }

    #[test]
    fn parses_openapi_attribute_body() {
        let body: OpenApiMetaBody = syn::parse2(quote!(
            description = "A todo",
            responses = (CreateTodoResponse, ValidationErrorResponse)
        ))
        .unwrap();

        assert_eq!(body.0.description.as_deref(), Some("A todo"));
        assert_eq!(body.0.responses.len(), 2);
    }

    #[test]
    fn defaults_to_get_when_only_a_path_is_given() {
        let parsed = args(quote!("/health"));
        assert!(matches!(parsed.method, Method::Get));
        assert_eq!(parsed.path.value(), "/health");
    }

    #[test]
    fn rejects_unknown_methods() {
        let error = syn::parse2::<RouteArgs>(quote!(FETCH "/")).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("unsupported HTTP method `FETCH`"),
            "{error}"
        );
    }

    #[test]
    fn rejects_trailing_tokens() {
        let error = syn::parse2::<RouteArgs>(quote!(GET "/" extra))
            .err()
            .unwrap();
        assert!(error.to_string().contains("unexpected tokens"), "{error}");
    }

    #[test]
    fn rejects_a_missing_path() {
        let error = syn::parse2::<RouteArgs>(quote!(GET)).err().unwrap();
        assert!(
            error.to_string().contains("expected a path string literal"),
            "{error}"
        );
    }

    #[test]
    fn rejects_unknown_openapi_keys() {
        let error = syn::parse2::<RouteArgs>(quote!(GET "/", nope = "x"))
            .err()
            .unwrap();
        assert!(error.to_string().contains("unknown openapi key"), "{error}");
    }

    #[test]
    fn rejects_non_async_handlers() {
        let error = expand(
            args(quote!(GET "/")),
            function(quote!(
                fn handler(ctx: Context) -> Response {
                    todo!()
                }
            )),
        )
        .unwrap_err();

        assert!(error.to_string().contains("must be `async fn`"), "{error}");
    }

    #[test]
    fn rejects_handlers_without_a_single_argument() {
        let error = expand(
            args(quote!(GET "/")),
            function(quote!(
                async fn handler() -> Response {
                    todo!()
                }
            )),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("exactly one argument"),
            "{error}"
        );
    }

    #[test]
    fn detects_typed_context_payloads() {
        let function = function(quote!(
            async fn handler(ctx: Context<CreateTodoRequest>) -> Response {
                let _ = ctx.req;
                todo!()
            }
        ));

        let payload = context_payload(&function.sig.inputs).unwrap();
        assert_eq!(quote!(#payload).to_string(), "CreateTodoRequest");
    }

    #[test]
    fn plain_context_has_no_payload() {
        let function = function(quote!(
            async fn handler(ctx: Context) -> Response {
                todo!()
            }
        ));

        assert!(context_payload(&function.sig.inputs).is_none());
    }

    #[test]
    fn generates_operation_when_metadata_is_present() {
        let tokens = expand(
            args(quote!(POST "/todos", summary = "Create", request = CreateTodoRequest)),
            function(quote!(
                async fn handler(ctx: Context) -> Response {
                    todo!()
                }
            )),
        )
        .unwrap()
        .to_string();

        assert!(tokens.contains("fn operation"), "{tokens}");
    }

    #[test]
    fn skips_operation_without_metadata() {
        let tokens = expand(
            args(quote!(GET "/")),
            function(quote!(
                async fn handler(ctx: Context) -> Response {
                    todo!()
                }
            )),
        )
        .unwrap()
        .to_string();

        assert!(!tokens.contains("fn operation"), "{tokens}");
    }
}
