use axum::{routing::get, Router};

mod error;
mod storage;

#[tokio::main]
async fn main() {
    // test code
    let store = storage::Storage::new();
    //

    // build our application with a single route
    /*
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run our app with hyper, listening globally on port 3000
    let addr = std::env::var("MEMFLOW_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
    */
}
