use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::pin::pin;
use std::sync::Arc;

use bytes::Bytes;
use http::StatusCode;
use http::header::{ALLOW, CONTENT_TYPE};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request as HttpRequest, Response as HttpResponse};
use hyper_util::rt::TokioIo;
use lumia_openapi::OpenApi;
use tokio::net::{TcpListener, ToSocketAddrs};

use crate::context::Context;
use crate::request::Request;
use crate::response::Response;
use crate::router::{Route, RouteMatch, Router};

/// Configuration for the OpenAPI document served alongside the routes.
#[derive(Debug, Clone)]
pub struct OpenApiEndpoint {
    /// The path the document is served at.
    pub path: String,
    /// The path the Scalar API reference is served at, when enabled.
    pub docs_path: Option<String>,
    /// The document metadata.
    pub document: OpenApi,
}

impl Default for OpenApiEndpoint {
    fn default() -> Self {
        Self {
            path: "/openapi.json".to_owned(),
            docs_path: Some("/docs".to_owned()),
            document: OpenApi::new("Lumia API", "0.0.0"),
        }
    }
}

/// The Lumia HTTP server.
///
/// ```text
/// let mut server = Server::new();
/// server.route(greet);
/// server.run("0.0.0.0:3000").await.unwrap();
/// ```
///
/// By default the server also serves an OpenAPI 3.0 document at
/// `/openapi.json`, built from every route annotated with `#[openapi(...)]`,
/// and a [Scalar](https://github.com/scalar/scalar) API reference at `/docs`.
pub struct Server {
    router: Router,
    openapi: Option<OpenApiEndpoint>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            router: Router::new(),
            openapi: Some(OpenApiEndpoint::default()),
        }
    }
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("router", &self.router)
            .finish()
    }
}

impl Server {
    /// Creates a server with no routes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a route.
    ///
    /// Handlers annotated with the `#[route]` attribute macro from
    /// `lumia-macros` can be passed directly, because the macro turns them into
    /// a value implementing [`Route`].
    ///
    /// # Panics
    ///
    /// Panics if a route with the same method and path template is already
    /// registered.
    pub fn route<R: Route>(&mut self, route: R) -> &mut Self {
        self.router.insert(route);
        self
    }

    /// The router backing this server.
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Replaces the metadata of the served OpenAPI document.
    ///
    /// The document is generated from every route annotated with
    /// `#[openapi(...)]` and served at `/openapi.json` by default.
    pub fn openapi(&mut self, document: OpenApi) -> &mut Self {
        match &mut self.openapi {
            Some(endpoint) => endpoint.document = document,
            None => {
                self.openapi = Some(OpenApiEndpoint {
                    document,
                    ..OpenApiEndpoint::default()
                })
            }
        }
        self
    }

    /// Changes the path the OpenAPI document is served at.
    ///
    /// Enables the document when it had been disabled.
    pub fn openapi_path(&mut self, path: impl Into<String>) -> &mut Self {
        let path = path.into();
        match &mut self.openapi {
            Some(endpoint) => endpoint.path = path,
            None => {
                self.openapi = Some(OpenApiEndpoint {
                    path,
                    ..OpenApiEndpoint::default()
                })
            }
        }
        self
    }

    /// Changes the path the Scalar API reference is served at.
    ///
    /// Enables the document and the reference when they had been disabled.
    pub fn docs_path(&mut self, path: impl Into<String>) -> &mut Self {
        let path = Some(path.into());
        match &mut self.openapi {
            Some(endpoint) => endpoint.docs_path = path,
            None => {
                self.openapi = Some(OpenApiEndpoint {
                    docs_path: path,
                    ..OpenApiEndpoint::default()
                })
            }
        }
        self
    }

    /// Stops serving the Scalar API reference while keeping the document.
    pub fn disable_docs(&mut self) -> &mut Self {
        if let Some(endpoint) = &mut self.openapi {
            endpoint.docs_path = None;
        }
        self
    }

    /// Stops serving the OpenAPI document.
    pub fn disable_openapi(&mut self) -> &mut Self {
        self.openapi = None;
        self
    }

    /// Binds to `addr` and serves requests until the process ends.
    pub async fn run(self, addr: impl ToSocketAddrs) -> io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        self.serve(listener).await
    }

    /// Binds to `addr` and serves requests until `shutdown` resolves.
    pub async fn run_until<F>(self, addr: impl ToSocketAddrs, shutdown: F) -> io::Result<()>
    where
        F: Future<Output = ()> + Send,
    {
        let listener = TcpListener::bind(addr).await?;
        self.serve_until(listener, shutdown).await
    }

    /// Serves requests on an already bound listener.
    ///
    /// Useful in tests, where the listener is bound to port `0` and the real
    /// address is read back from `TcpListener::local_addr`.
    pub async fn serve(self, listener: TcpListener) -> io::Result<()> {
        self.serve_until(listener, std::future::pending()).await
    }

    /// Serves requests on an already bound listener until `shutdown` resolves.
    pub async fn serve_until<F>(self, listener: TcpListener, shutdown: F) -> io::Result<()>
    where
        F: Future<Output = ()> + Send,
    {
        let router = Arc::new(self.router);
        let openapi = Arc::new(self.openapi);
        let mut shutdown = pin!(shutdown);

        loop {
            tokio::select! {
                () = &mut shutdown => return Ok(()),
                accepted = listener.accept() => {
                    let (stream, _peer) = accepted?;
                    let router = Arc::clone(&router);
                    let openapi = Arc::clone(&openapi);
                    tokio::spawn(async move {
                        let service = service_fn(move |request| {
                            let router = Arc::clone(&router);
                            let openapi = Arc::clone(&openapi);
                            async move {
                                let openapi = (*openapi).as_ref();
                                Ok::<_, Infallible>(dispatch(&router, openapi, request).await)
                            }
                        });

                        if let Err(error) = http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .await
                        {
                            eprintln!("lumia: connection error: {error}");
                        }
                    });
                }
            }
        }
    }
}

async fn dispatch(
    router: &Router,
    openapi: Option<&OpenApiEndpoint>,
    request: HttpRequest<Incoming>,
) -> HttpResponse<Full<Bytes>> {
    if let Some(endpoint) = openapi {
        let method = request.method();
        if *method == Method::GET || *method == Method::HEAD {
            let path = request.uri().path();

            if path == endpoint.path {
                let response = if *method == Method::HEAD {
                    Response::empty()
                } else {
                    let operations = router.operations();
                    Response::new(endpoint.document.to_json(&operations))
                };
                return response
                    .with_header(CONTENT_TYPE.as_str(), "application/json; charset=utf-8")
                    .into_hyper();
            }

            if endpoint.docs_path.as_deref() == Some(path) {
                let body = if *method == Method::HEAD {
                    String::new()
                } else {
                    lumia_openapi::scalar_html(&endpoint.path)
                };
                return Response::html(body).into_hyper();
            }
        }
    }

    let (parts, body) = request.into_parts();

    let body = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(error) => {
            eprintln!("lumia: failed to read request body: {error}");
            return Response::text("failed to read request body")
                .with_status(StatusCode::BAD_REQUEST)
                .into_hyper();
        }
    };

    match router.at(&parts.method, parts.uri.path()) {
        RouteMatch::Found { route, params } => {
            let context = Context::new(Request::new(
                parts.method,
                parts.uri,
                parts.headers,
                params,
                body,
            ));

            route.call(context).await.into_hyper()
        }
        RouteMatch::MethodNotAllowed(methods) => {
            let allow = methods
                .iter()
                .map(|method| method.as_str())
                .collect::<Vec<_>>()
                .join(", ");

            Response::text("method not allowed")
                .with_status(StatusCode::METHOD_NOT_ALLOWED)
                .with_header(ALLOW.as_str(), allow)
                .into_hyper()
        }
        RouteMatch::NotFound => Response::text("not found")
            .with_status(StatusCode::NOT_FOUND)
            .into_hyper(),
    }
}

impl Response {
    pub(crate) fn into_hyper(self) -> HttpResponse<Full<Bytes>> {
        let status = self.status();
        let headers = self.headers().clone();
        let body = self.into_body();

        let mut response = HttpResponse::new(Full::new(body));
        *response.status_mut() = status;
        *response.headers_mut() = headers;
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::BoxFuture;

    struct Echo {
        method: http::Method,
        path: &'static str,
    }

    impl Route for Echo {
        fn method(&self) -> http::Method {
            self.method.clone()
        }

        fn path(&self) -> &'static str {
            self.path
        }

        fn call(&self, ctx: Context) -> BoxFuture<'static, Response> {
            Box::pin(async move {
                Response::json(serde_json::json!({
                    "id": ctx.param("id"),
                    "q": ctx.query("q"),
                    "method": ctx.method().as_str(),
                }))
            })
        }
    }

    #[test]
    fn server_registers_routes() {
        let mut server = Server::new();
        server
            .route(Echo {
                method: http::Method::GET,
                path: "/",
            })
            .route(Echo {
                method: http::Method::POST,
                path: "/things",
            });

        assert_eq!(server.router().len(), 2);
    }
}
