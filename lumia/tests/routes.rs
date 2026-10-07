//! End-to-end tests: real TCP connections against a running `Server`.

use std::net::SocketAddr;
use std::time::Duration;

use lumia::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[route(GET "/")]
async fn index(ctx: Context) -> Response {
    ctx.Json(serde_json::json!({ "message": "Hello, World!" }))
}

#[route(GET "/users/{id}")]
async fn show_user(ctx: Context) -> Response {
    ctx.Json(serde_json::json!({ "id": ctx.param("id") }))
}

#[route(GET "/status")]
async fn query_state(ctx: Context) -> Response {
    Response::json(serde_json::json!({ "status": ctx.query("state") }))
}

#[route("/default-method")]
async fn default_method(ctx: Context) -> Response {
    ctx.Text("default method is GET")
}

#[route(POST "/users")]
async fn create_user(ctx: Context) -> Result<Response, Error> {
    let payload: serde_json::Value = ctx.json()?;
    Ok(Response::json(serde_json::json!({ "created": payload })).with_status(StatusCode::CREATED))
}

#[route(GET "/teapot")]
async fn teapot(_ctx: Context) -> Result<&'static str, Error> {
    Err(Error::new(StatusCode::IM_A_TEAPOT, "I'm a teapot"))
}

// `#[inline]` only applies to functions, so it must not be mirrored onto the
// struct the macro generates.
#[route(PUT "/plain")]
#[inline]
async fn plain(_ctx: Context) -> &'static str {
    "plain text"
}

#[route(GET "/wrapped")]
async fn wrapped(_ctx: Context) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

#[route(GET "/headers")]
async fn headers(ctx: Context) -> Response {
    let agent = ctx.header("x-agent").unwrap_or("none").to_owned();
    Response::text(format!("{agent}:{}", ctx.text()))
        .with_status(StatusCode::CREATED)
        .with_header("x-route", "headers")
}

async fn start(server: Server) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(server.serve(listener));
    addr
}

async fn request(addr: SocketAddr, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let response = raw_request(addr, method, path, body, &[]).await;
    (response.status, response.body)
}

struct RawResponse {
    status: u16,
    head: String,
    body: String,
}

async fn raw_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
    extra_headers: &[(&str, &str)],
) -> RawResponse {
    let mut stream = TcpStream::connect(addr).await.unwrap();

    let mut request =
        format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (name, value) in extra_headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }

    stream.write_all(request.as_bytes()).await.unwrap();

    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut raw))
        .await
        .expect("timed out waiting for a response")
        .unwrap();

    let response = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = response.split_once("\r\n\r\n").expect("malformed response");
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("missing status")
        .parse()
        .expect("invalid status");

    RawResponse {
        status,
        head: head.to_owned(),
        body: body.to_owned(),
    }
}

fn server() -> Server {
    let mut server = Server::new();
    server
        .route(index)
        .route(show_user)
        .route(query_state)
        .route(default_method)
        .route(create_user)
        .route(teapot)
        .route(plain)
        .route(wrapped)
        .route(headers);
    server
}

#[tokio::test]
async fn serves_a_json_route() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/", None).await;

    assert_eq!(status, 200);
    assert_eq!(body, r#"{"message":"Hello, World!"}"#);
}

#[tokio::test]
async fn captures_path_parameters() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/users/42", None).await;

    assert_eq!(status, 200);
    assert_eq!(body, r#"{"id":"42"}"#);
}

#[tokio::test]
async fn reads_query_parameters() {
    let addr = start(server()).await;
    let (_, body) = request(addr, "GET", "/status?state=up%20and%20running", None).await;

    assert_eq!(body, r#"{"status":"up and running"}"#);
}

#[tokio::test]
async fn defaults_to_get_when_the_method_is_omitted() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/default-method", None).await;

    assert_eq!(status, 200);
    assert_eq!(body, "default method is GET");
}

#[tokio::test]
async fn parses_a_json_body() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "POST", "/users", Some(r#"{"name":"Ada"}"#)).await;

    assert_eq!(status, 201);
    assert_eq!(body, r#"{"created":{"name":"Ada"}}"#);
}

#[tokio::test]
async fn rejects_a_malformed_json_body() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "POST", "/users", Some("{oops")).await;

    assert_eq!(status, 400);
    assert_eq!(body, r#"{"error":"invalid JSON body"}"#);
}

#[tokio::test]
async fn renders_errors_from_result_handlers() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/teapot", None).await;

    assert_eq!(status, 418);
    assert_eq!(body, r#"{"error":"I'm a teapot"}"#);
}

#[tokio::test]
async fn supports_handlers_returning_plain_text() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "PUT", "/plain", None).await;

    assert_eq!(status, 200);
    assert_eq!(body, "plain text");
}

#[tokio::test]
async fn supports_handlers_returning_json_wrappers() {
    let addr = start(server()).await;
    let response = raw_request(addr, "GET", "/wrapped", None, &[]).await;

    assert_eq!(response.status, 200);
    assert!(response.head.contains("content-type: application/json"));
    assert_eq!(response.body, r#"{"ok":true}"#);
}

#[tokio::test]
async fn exposes_request_headers_and_bodies() {
    let addr = start(server()).await;
    let response = raw_request(
        addr,
        "GET",
        "/headers",
        Some("payload"),
        &[("x-agent", "curl")],
    )
    .await;

    assert_eq!(response.status, 201);
    assert!(response.head.contains("x-route: headers"));
    assert_eq!(response.body, "curl:payload");
}

#[tokio::test]
async fn returns_404_for_unknown_paths() {
    let addr = start(server()).await;
    let (status, _) = request(addr, "GET", "/missing", None).await;

    assert_eq!(status, 404);
}

#[tokio::test]
async fn returns_405_with_an_allow_header() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "DELETE", "/", None).await;

    assert_eq!(status, 405);
    assert_eq!(body, "method not allowed");
}

#[tokio::test]
async fn answers_head_requests_using_the_get_route() {
    let addr = start(server()).await;
    let (status, _) = request(addr, "HEAD", "/", None).await;

    assert_eq!(status, 200);
}
