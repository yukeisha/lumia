use lumia::prelude::*;

#[tokio::main]
async fn main() {
    let mut server = Server::new();
    server.openapi(OpenApi::new("Todo API", "0.1.0"));
    server.route(greet);
    server.run("0.0.0.0:3000").await.unwrap();
}

#[route(POST "/todos")]
#[openapi(
    summary = "Create a new todo",
    description = "Create a new todo ...",
    tag = "Todo",
    request = CreateTodoRequest,
    responses = (
        CreateTodoResponse, // Custom success response (via derive)
        ValidationErrorResponse, // Builtin error response
        InternalErrorResponse // Builtin error response
    )
)]
async fn greet(ctx: Context<CreateTodoRequest>) -> Response {
    CreateTodoResponse::builder()
        .title(ctx.req.title)
        .description(ctx.req.description)
        .status(ctx.req.status)
        .build()
        .into_response()
}

#[derive(Debug, Deserialize, Schema)]
struct CreateTodoRequest {
    title: String,
    description: String,
    status: TodoStatus,
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
