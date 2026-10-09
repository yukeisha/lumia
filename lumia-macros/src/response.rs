//! `#[derive(Response)]`: a builder, `IntoResponse`, `ApiResponse` and a schema.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Attribute, Data, DeriveInput, Error, Expr, ExprLit, Fields, Lit, Meta, Token};

use crate::schema::schema_impl;

struct ResponseArgs {
    status: u16,
    description: Option<String>,
}

impl Default for ResponseArgs {
    fn default() -> Self {
        Self {
            status: 200,
            description: None,
        }
    }
}

/// Expands the full response implementation for the input type.
pub fn response_impl(input: &DeriveInput) -> syn::Result<TokenStream> {
    if input.generics.params.iter().next().is_some() || input.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &input.ident,
            "Response cannot be derived for generic types",
        ));
    }

    let name = &input.ident;
    let name_str = name.to_string();
    let vis = &input.vis;
    let args = response_args(&input.attrs);
    let status = args.status;
    let description = args
        .description
        .unwrap_or_else(|| reason_phrase(status).to_owned());

    let fields: Vec<&syn::Field> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect(),
            Fields::Unit => Vec::new(),
            Fields::Unnamed(unnamed) => {
                return Err(Error::new_spanned(
                    unnamed,
                    "Response can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(Error::new_spanned(
                name,
                "Response can only be derived for structs",
            ));
        }
    };

    let builder = format_ident!("{}Builder", name);
    let field_idents: Vec<&syn::Ident> = fields
        .iter()
        .map(|field| field.ident.as_ref().expect("named fields have identifiers"))
        .collect();
    let field_types: Vec<&syn::Type> = fields.iter().map(|field| &field.ty).collect();

    let schema = schema_impl(input)?;

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #vis struct #builder {
            #( #field_idents: ::std::option::Option<#field_types> ),*
        }

        impl ::std::default::Default for #builder {
            fn default() -> Self {
                Self {
                    #( #field_idents: ::std::option::Option::None ),*
                }
            }
        }

        impl #name {
            /// Creates a builder for this response.
            #vis fn builder() -> #builder {
                <#builder as ::std::default::Default>::default()
            }
        }

        impl #builder {
            #(
                #vis fn #field_idents(
                    mut self,
                    value: impl ::std::convert::Into<#field_types>,
                ) -> Self {
                    self.#field_idents =
                        ::std::option::Option::Some(::std::convert::Into::into(value));
                    self
                }
            )*

            #vis fn build(self) -> #name {
                #name {
                    #(
                        #field_idents: match self.#field_idents {
                            ::std::option::Option::Some(value) => value,
                            ::std::option::Option::None => ::std::panic!(
                                concat!(
                                    "field `",
                                    stringify!(#field_idents),
                                    "` was not set on ",
                                    stringify!(#name),
                                )
                            ),
                        }
                    ),*
                }
            }
        }

        impl ::lumia::IntoResponse for #name {
            fn into_response(self) -> ::lumia::Response {
                let status = <#name as ::lumia::openapi::ApiResponse>::status_code();
                ::lumia::Response::json(self).with_status(
                    ::lumia::StatusCode::from_u16(status).unwrap_or(::lumia::StatusCode::OK),
                )
            }
        }

        impl ::lumia::openapi::ApiResponse for #name {
            fn status_code() -> u16 {
                #status
            }

            fn description() -> &'static str {
                #description
            }

            fn schema() -> ::std::option::Option<::lumia::openapi::Schema> {
                ::std::option::Option::Some(::lumia::openapi::Schema::named(
                    ::std::string::String::from(#name_str),
                    <#name as ::lumia::openapi::OpenApiSchema>::schema(),
                ))
            }
        }

        #schema
    })
}

fn response_args(attrs: &[Attribute]) -> ResponseArgs {
    let mut args = ResponseArgs::default();

    for attr in attrs {
        if !attr.path().is_ident("response") {
            continue;
        }
        let Meta::List(list) = &attr.meta else {
            continue;
        };
        let Ok(items) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            continue;
        };

        for item in items {
            let Meta::NameValue(name_value) = item else {
                continue;
            };
            if name_value.path.is_ident("status") {
                if let Some(status) = status_value(&name_value.value) {
                    args.status = status;
                }
            } else if name_value.path.is_ident("description")
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(value),
                    ..
                }) = &name_value.value
            {
                args.description = Some(value.value());
            }
        }
    }

    args
}

fn status_value(expr: &Expr) -> Option<u16> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value.base10_parse().ok(),
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => value.value().parse().ok(),
        _ => None,
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No content",
        400 => "Bad request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not found",
        409 => "Conflict",
        422 => "Unprocessable entity",
        500 => "Internal server error",
        502 => "Bad gateway",
        503 => "Service unavailable",
        _ => "Response",
    }
}
