//! Attribute macros for Lumia.
//!
//! The main entry point is [`route`], which turns an async function into a
//! route that can be passed to `Server::route`.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Error, Ident, ItemFn, LitStr, Token};

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

fn expand(args: RouteArgs, function: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
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

    let name = function.sig.ident.clone();
    let handler = Ident::new(&format!("__lumia_handler_{name}"), span);
    let method = args.method.variant();
    let path = args.path;
    let attrs = &function.attrs;
    let mirrored = mirror_attrs(attrs);
    let vis = &function.vis;
    let mut signature = function.sig.clone();
    signature.ident = handler.clone();
    let body = &function.block;

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
                    ::lumia::IntoResponse::into_response(#handler(ctx).await)
                })
            }
        }

        #(#attrs)*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #signature #body
    })
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
}
impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitStr) {
            let path = input.parse()?;
            finish(input)?;
            return Ok(Self {
                method: Method::Get,
                path,
            });
        }

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
        finish(input)?;

        Ok(Self { method, path })
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
        let name = match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Connect => "CONNECT",
        };
        Ident::new(name, proc_macro2::Span::call_site())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn mirror(input: proc_macro2::TokenStream) -> Vec<String> {
        use syn::parse::Parser;

        let attrs = syn::Attribute::parse_outer.parse2(input).unwrap();
        mirror_attrs(&attrs)
            .iter()
            .map(|attr| attr.path().get_ident().unwrap().to_string())
            .collect()
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
        let args = syn::parse2::<RouteArgs>(quote!(POST "/users")).unwrap();
        assert!(matches!(args.method, Method::Post));
        assert_eq!(args.path.value(), "/users");
    }

    #[test]
    fn parses_comma_separated_and_lowercase_forms() {
        let args = syn::parse2::<RouteArgs>(quote!(delete, "/users/{id}")).unwrap();
        assert!(matches!(args.method, Method::Delete));
        assert_eq!(args.path.value(), "/users/{id}");
    }

    #[test]
    fn defaults_to_get_when_only_a_path_is_given() {
        let args = syn::parse2::<RouteArgs>(quote!("/health")).unwrap();
        assert!(matches!(args.method, Method::Get));
        assert_eq!(args.path.value(), "/health");
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
    fn rejects_non_async_handlers() {
        let function = syn::parse2::<ItemFn>(quote!(
            fn handler(ctx: Context) -> Response {
                todo!()
            }
        ))
        .unwrap();
        let error = expand(
            RouteArgs {
                method: Method::Get,
                path: LitStr::new("/", proc_macro2::Span::call_site()),
            },
            function,
        )
        .unwrap_err();

        assert!(error.to_string().contains("must be `async fn`"), "{error}");
    }

    #[test]
    fn rejects_handlers_without_a_single_argument() {
        let function = syn::parse2::<ItemFn>(quote!(
            async fn handler() -> Response {
                todo!()
            }
        ))
        .unwrap();
        let error = expand(
            RouteArgs {
                method: Method::Get,
                path: LitStr::new("/", proc_macro2::Span::call_site()),
            },
            function,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("exactly one argument"),
            "{error}"
        );
    }
}
