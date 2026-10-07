use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::pin::pin;
use std::sync::Arc;

use bytes::Bytes;
use http::StatusCode;
use http::header::ALLOW;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request as HttpRequest, Response as HttpResponse};
use hyper_util::rt::TokioIo;
use tokio::net::{TcpListener, ToSocketAddrs};

use crate::context::Context;
use crate::request::Request;
use crate::response::Response;
use crate::router::{Route, RouteMatch, Router};

/// The Lumia HTTP server.
///
/// ```text
/// let mut server = Server::new();
/// server.route(greet);
/// server.run("0.0.0.0:3000").await.unwrap();
/// ```
#[derive(Default)]
pub struct Server {
    router: Router,
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
        let mut shutdown = pin!(shutdown);

        loop {
            tokio::select! {
                () = &mut shutdown => return Ok(()),
                accepted = listener.accept() => {
                    let (stream, _peer) = accepted?;
                    let router = Arc::clone(&router);
                    tokio::spawn(async move {
                        let service = service_fn(move |request| {
                            let router = Arc::clone(&router);
                            async move { Ok::<_, Infallible>(dispatch(&router, request).await) }
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

async fn dispatch(router: &Router, request: HttpRequest<Incoming>) -> HttpResponse<Full<Bytes>> {
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
