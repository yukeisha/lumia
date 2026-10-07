use lumia::prelude::*;

#[tokio::main]
async fn main() {
    let mut server = Server::new();
    server.route(greet);
    server.run("0.0.0.0:3000").await.unwrap();
}

#[route(GET "/")]
async fn greet(ctx: Context) -> Response {
    ctx.Json(serde_json::json!({
        "message": "Hello, World!"
    }))
}
