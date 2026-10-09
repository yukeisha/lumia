use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http::Method;

use crate::context::Context;
use crate::response::Response;
use crate::util::decode_path_segment;

/// A boxed, sendable future returned by a route handler.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A single route: an HTTP method, a path template and a handler.
///
/// The `#[route]` attribute macro from `lumia-macros` generates an implementation of
/// this trait for every annotated handler, which is what lets
/// [`Server::route`](crate::Server::route) accept the handler by name:
///
/// ```text
/// #[route(GET "/")]
/// async fn greet(ctx: Context) -> Response { ... }
///
/// server.route(greet);
/// ```
pub trait Route: Send + Sync + 'static {
    /// The HTTP method this route responds to.
    fn method(&self) -> Method;

    /// The path template, for example `/users/{id}`.
    fn path(&self) -> &'static str;

    /// Invokes the handler.
    fn call(&self, ctx: Context) -> BoxFuture<'static, Response>;

    /// OpenAPI metadata for this route, when it was declared.
    ///
    /// The `#[openapi(...)]` attribute on a handler makes the `#[route]` macro
    /// implement this method; routes without the attribute return `None` and
    /// are omitted from the generated document.
    fn operation(&self) -> Option<lumia_openapi::Operation> {
        None
    }
}

/// The outcome of matching a request against the router.
#[derive(Clone)]
pub enum RouteMatch {
    /// A route matched; contains the route and its captured parameters.
    Found {
        /// The matched route.
        route: Arc<dyn Route>,
        /// Path parameters captured from the request path.
        params: HashMap<String, String>,
    },
    /// The path matched, but no route handles this method.
    MethodNotAllowed(Vec<Method>),
    /// No route matched the path.
    NotFound,
}

impl std::fmt::Debug for RouteMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Found { params, .. } => f
                .debug_struct("Found")
                .field("params", params)
                .finish_non_exhaustive(),
            Self::MethodNotAllowed(methods) => {
                f.debug_tuple("MethodNotAllowed").field(methods).finish()
            }
            Self::NotFound => f.write_str("NotFound"),
        }
    }
}

#[derive(Clone)]
enum Segment {
    Literal(String),
    Param(String),
    CatchAll(String),
}

struct Compiled {
    method: Method,
    template: &'static str,
    segments: Vec<Segment>,
    route: Arc<dyn Route>,
}

/// Matches request paths and methods against registered routes.
#[derive(Default)]
pub struct Router {
    routes: Vec<Compiled>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("routes", &self.routes.len())
            .finish()
    }
}

impl Router {
    /// Creates an empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a route.
    ///
    /// # Panics
    ///
    /// Panics if a route with the same method and path template is already
    /// registered.
    pub fn insert<R: Route>(&mut self, route: R) {
        let method = route.method();
        let template = route.path();
        let segments = compile(template);

        if self
            .routes
            .iter()
            .any(|existing| existing.method == method && existing.template == template)
        {
            panic!("duplicate route registered: {method} {template}");
        }

        self.routes.push(Compiled {
            method,
            template,
            segments,
            route: Arc::new(route),
        });
    }

    /// The number of registered routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// OpenAPI metadata for every route that declared it.
    pub fn operations(&self) -> Vec<lumia_openapi::Operation> {
        self.routes
            .iter()
            .filter_map(|route| route.route.operation())
            .collect()
    }

    /// Returns `true` when no routes are registered.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Finds the route matching `method` and `path`.
    pub fn at(&self, method: &Method, path: &str) -> RouteMatch {
        let segments = split_path(path);
        let mut allowed = Vec::new();

        for candidate in &self.routes {
            let Some(params) = match_segments(&candidate.segments, &segments) else {
                continue;
            };

            if candidate.method == *method
                || (*method == Method::HEAD && candidate.method == Method::GET)
            {
                return RouteMatch::Found {
                    route: Arc::clone(&candidate.route),
                    params,
                };
            }

            if !allowed.contains(&candidate.method) {
                allowed.push(candidate.method.clone());
            }
        }

        if allowed.is_empty() {
            RouteMatch::NotFound
        } else {
            if allowed.contains(&Method::GET) && !allowed.contains(&Method::HEAD) {
                allowed.push(Method::HEAD);
            }
            RouteMatch::MethodNotAllowed(allowed)
        }
    }
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn compile(template: &str) -> Vec<Segment> {
    split_path(template)
        .into_iter()
        .map(|segment| {
            if let Some(name) = segment
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
            {
                if let Some(name) = name.strip_prefix('*') {
                    Segment::CatchAll(name.to_owned())
                } else {
                    Segment::Param(name.to_owned())
                }
            } else if let Some(name) = segment.strip_prefix(':') {
                Segment::Param(name.to_owned())
            } else if let Some(name) = segment.strip_prefix('*') {
                Segment::CatchAll(name.to_owned())
            } else {
                Segment::Literal(segment.to_owned())
            }
        })
        .collect()
}

fn match_segments(template: &[Segment], path: &[&str]) -> Option<HashMap<String, String>> {
    let mut params = HashMap::new();
    if walk(template, path, &mut params) {
        Some(params)
    } else {
        None
    }
}

fn walk(template: &[Segment], path: &[&str], params: &mut HashMap<String, String>) -> bool {
    match template.split_first() {
        None => path.is_empty(),
        Some((Segment::CatchAll(name), _)) => {
            params.insert(name.clone(), decode_path_segment(&path.join("/")));
            true
        }
        Some((segment, rest)) => {
            let Some((value, remaining)) = path.split_first() else {
                return false;
            };

            match segment {
                Segment::Literal(expected) => expected == value && walk(rest, remaining, params),
                Segment::Param(name) => {
                    let decoded = decode_path_segment(value);
                    let previous = params.insert(name.clone(), decoded);
                    if walk(rest, remaining, params) {
                        true
                    } else {
                        match previous {
                            Some(previous) => params.insert(name.clone(), previous),
                            None => params.remove(name),
                        };
                        false
                    }
                }
                Segment::CatchAll(_) => unreachable!("handled above"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::Response;

    struct TestRoute {
        method: Method,
        path: &'static str,
    }

    impl Route for TestRoute {
        fn method(&self) -> Method {
            self.method.clone()
        }

        fn path(&self) -> &'static str {
            self.path
        }

        fn call(&self, _ctx: Context) -> BoxFuture<'static, Response> {
            Box::pin(async { Response::empty() })
        }
    }

    fn router() -> Router {
        let mut router = Router::new();
        router.insert(TestRoute {
            method: Method::GET,
            path: "/",
        });
        router.insert(TestRoute {
            method: Method::GET,
            path: "/users",
        });
        router.insert(TestRoute {
            method: Method::POST,
            path: "/users",
        });
        router.insert(TestRoute {
            method: Method::GET,
            path: "/users/{id}",
        });
        router.insert(TestRoute {
            method: Method::GET,
            path: "/files/*path",
        });
        router
    }

    fn params(result: RouteMatch) -> HashMap<String, String> {
        match result {
            RouteMatch::Found { params, .. } => params,
            other => panic!("expected a match, got {other:?}"),
        }
    }

    #[test]
    fn matches_static_paths() {
        assert_eq!(params(router().at(&Method::GET, "/")).len(), 0);
        assert_eq!(params(router().at(&Method::GET, "/users")).len(), 0);
    }

    #[test]
    fn tolerates_trailing_slashes() {
        assert!(matches!(
            router().at(&Method::GET, "/users/"),
            RouteMatch::Found { .. }
        ));
    }

    #[test]
    fn captures_path_parameters() {
        let params = params(router().at(&Method::GET, "/users/42"));
        assert_eq!(params.get("id").map(String::as_str), Some("42"));
    }

    #[test]
    fn supports_colon_parameters() {
        let mut router = Router::new();
        router.insert(TestRoute {
            method: Method::GET,
            path: "/items/:id",
        });

        let params = params(router.at(&Method::GET, "/items/9"));
        assert_eq!(params.get("id").map(String::as_str), Some("9"));
    }

    #[test]
    fn supports_bare_catch_all() {
        let mut router = Router::new();
        router.insert(TestRoute {
            method: Method::GET,
            path: "/assets/*path",
        });

        let params = params(router.at(&Method::GET, "/assets/css/app.css"));
        assert_eq!(params.get("path").map(String::as_str), Some("css/app.css"));
    }

    #[test]
    fn decodes_path_parameters() {
        let params = params(router().at(&Method::GET, "/users/Jane%20Doe"));
        assert_eq!(params.get("id").map(String::as_str), Some("Jane Doe"));
    }

    #[test]
    fn captures_catch_all_tail() {
        let params = params(router().at(&Method::GET, "/files/a/b/c.txt"));
        assert_eq!(params.get("path").map(String::as_str), Some("a/b/c.txt"));
    }

    #[test]
    fn rejects_unknown_paths() {
        assert!(matches!(
            router().at(&Method::GET, "/nope"),
            RouteMatch::NotFound
        ));
        assert!(matches!(
            router().at(&Method::GET, "/users/42/extra"),
            RouteMatch::NotFound
        ));
    }

    #[test]
    fn reports_method_not_allowed() {
        match router().at(&Method::DELETE, "/users") {
            RouteMatch::MethodNotAllowed(methods) => {
                assert!(methods.contains(&Method::GET));
                assert!(methods.contains(&Method::POST));
            }
            other => panic!("expected 405, got {other:?}"),
        }
    }

    #[test]
    fn head_falls_back_to_get() {
        assert!(matches!(
            router().at(&Method::HEAD, "/users"),
            RouteMatch::Found { .. }
        ));
    }

    #[test]
    fn lists_allow_header_including_head() {
        match router().at(&Method::PATCH, "/users") {
            RouteMatch::MethodNotAllowed(methods) => assert!(methods.contains(&Method::HEAD)),
            other => panic!("expected 405, got {other:?}"),
        }
    }

    #[test]
    #[should_panic(expected = "duplicate route registered")]
    fn panics_on_duplicate_routes() {
        let mut router = Router::new();
        router.insert(TestRoute {
            method: Method::GET,
            path: "/dup",
        });
        router.insert(TestRoute {
            method: Method::GET,
            path: "/dup",
        });
    }
}
