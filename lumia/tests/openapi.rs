//! End-to-end tests for typed contexts, derives and the OpenAPI document.

use std::net::SocketAddr;
use std::time::Duration;

use lumia::openapi::OpenApi;
use lumia::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Deserialize, Schema)]
struct CreateTodoRequest {
    title: String,
    description: String,
    status: TodoStatus,
    note: Option<String>,
}

#[derive(Debug, Serialize, Response)]
#[response(status = 201, description = "Todo created")]
struct CreateTodoResponse {
    title: String,
    description: String,
    status: TodoStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

#[route(POST "/todos")]
#[openapi(
    summary = "Create a new todo",
    description = "Create a new todo ...",
    tag = "Todo",
    request = CreateTodoRequest,
    responses = (
        CreateTodoResponse,
        ValidationErrorResponse,
        InternalErrorResponse
    )
)]
async fn create_todo(ctx: Context<CreateTodoRequest>) -> Response {
    let _ = ctx.req.note.as_deref();
    CreateTodoResponse::builder()
        .title(ctx.req.title)
        .description(ctx.req.description)
        .status(ctx.req.status)
        .build()
        .into_response()
}

#[route(GET "/plain")]
async fn plain(ctx: Context) -> Response {
    ctx.Text("ok")
}

#[openapi(
    summary = "List todos",
    tag = "Todo",
    responses = (ValidationErrorResponse)
)]
#[route(GET "/todos")]
async fn list_todos(ctx: Context) -> Response {
    ctx.Json(serde_json::json!([]))
}

async fn start(server: Server) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(server.serve(listener));
    addr
}

async fn request(addr: SocketAddr, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.unwrap();

    let mut raw = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    if let Some(body) = body {
        raw.push_str("Content-Type: application/json\r\n");
        raw.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    raw.push_str("\r\n");
    if let Some(body) = body {
        raw.push_str(body);
    }
    stream.write_all(raw.as_bytes()).await.unwrap();

    let mut buffer = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut buffer))
        .await
        .expect("timed out waiting for a response")
        .unwrap();

    let response = String::from_utf8_lossy(&buffer).into_owned();
    let (head, body) = response.split_once("\r\n\r\n").expect("malformed response");
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("missing status")
        .parse()
        .expect("invalid status");

    (status, body.to_owned())
}

fn server() -> Server {
    let mut server = Server::new();
    server.openapi(OpenApi::new("Todo API", "0.1.0"));
    server.route(create_todo).route(plain).route(list_todos);
    server
}

#[tokio::test]
async fn deserializes_a_typed_request_body() {
    let addr = start(server()).await;
    let (status, body) = request(
        addr,
        "POST",
        "/todos",
        Some(r#"{"title":"Ship it","description":"soon","status":"in_progress"}"#),
    )
    .await;

    assert_eq!(status, 201);
    assert_eq!(
        body,
        r#"{"title":"Ship it","description":"soon","status":"in_progress"}"#
    );
}

#[tokio::test]
async fn rejects_a_body_that_does_not_match_the_type() {
    let addr = start(server()).await;
    let (status, _) = request(addr, "POST", "/todos", Some(r#"{"title":"x"}"#)).await;

    assert_eq!(status, 400);
}

#[tokio::test]
async fn serves_the_openapi_document() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/openapi.json", None).await;

    assert_eq!(status, 200);
    let document: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(document["info"]["title"], "Todo API");
    assert_eq!(document["info"]["version"], "0.1.0");
    assert_eq!(
        document["paths"]["/todos"]["post"]["summary"],
        "Create a new todo"
    );
    assert_eq!(
        document["paths"]["/todos"]["post"]["requestBody"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/CreateTodoRequest"
    );
    assert_eq!(
        document["paths"]["/todos"]["post"]["responses"]["201"]["description"],
        "Todo created"
    );
    assert_eq!(
        document["paths"]["/todos"]["post"]["responses"]["400"]["description"],
        "Validation error"
    );
    assert_eq!(
        document["components"]["schemas"]["CreateTodoRequest"]["required"],
        serde_json::json!(["title", "description", "status"])
    );
    assert_eq!(
        document["components"]["schemas"]["CreateTodoRequest"]["properties"]["status"]["enum"],
        serde_json::json!(["pending", "in_progress", "done"])
    );
    assert_eq!(
        document["components"]["schemas"]["CreateTodoRequest"]["properties"]["note"]["nullable"],
        true
    );
    assert_eq!(document["paths"]["/todos"]["get"]["summary"], "List todos");
}

#[tokio::test]
async fn omits_routes_without_openapi_metadata() {
    let addr = start(server()).await;
    let (_, body) = request(addr, "GET", "/openapi.json", None).await;
    let document: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert!(document["paths"]["/plain"].is_null());
}

#[tokio::test]
async fn serves_the_scalar_api_reference() {
    let addr = start(server()).await;
    let (status, body) = request(addr, "GET", "/docs", None).await;

    assert_eq!(status, 200);
    assert!(body.contains(r#"data-url="/openapi.json""#), "{body}");
    assert!(
        body.contains("cdn.jsdelivr.net/npm/@scalar/api-reference"),
        "{body}"
    );
}

#[tokio::test]
async fn the_docs_path_is_configurable_and_can_be_disabled() {
    let mut server = Server::new();
    server.docs_path("/reference").route(plain);
    let addr = start(server).await;

    assert_eq!(request(addr, "GET", "/docs", None).await.0, 404);
    assert_eq!(request(addr, "GET", "/reference", None).await.0, 200);

    let mut server = Server::new();
    server.disable_docs().route(plain);
    let addr = start(server).await;

    assert_eq!(request(addr, "GET", "/docs", None).await.0, 404);
    assert_eq!(request(addr, "GET", "/openapi.json", None).await.0, 200);
}

#[tokio::test]
async fn the_document_can_be_moved_and_disabled() {
    let mut server = Server::new();
    server.openapi_path("/spec.json").disable_openapi();
    server.route(plain);
    let addr = start(server).await;

    assert_eq!(request(addr, "GET", "/spec.json", None).await.0, 404);
    assert_eq!(request(addr, "GET", "/docs", None).await.0, 404);

    let mut server = Server::new();
    server.openapi_path("/spec.json").route(plain);
    let addr = start(server).await;

    assert_eq!(request(addr, "GET", "/spec.json", None).await.0, 200);
}
