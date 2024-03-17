use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use bytes::BytesMut;
use error::ResponseResult;
use log::info;

mod error;
mod storage;
use storage::{plugin_analyzer, Storage};

#[tokio::main]
async fn main() {
    let store = Storage::new();

    // build our application with a single route
    let app = Router::new()
        .route("/", post(plugin_push))
        .with_state(store)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)); // 20 mb

    // run our app with hyper, listening globally on port 3000
    let addr = std::env::var("MEMFLOW_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn plugin_push(
    State(storage): State<Storage>,
    mut multipart: Multipart,
) -> ResponseResult<()> {
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
    {
        let name = field.name().unwrap().to_string();
        println!("name: {}", name);
        let file_name = field.file_name().unwrap().to_string();
        println!("file_name: {}", file_name);

        // read the buffer
        let mut data = BytesMut::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
        {
            data.extend_from_slice(&chunk);

            // check if this file is a potential binary or early abort
            if data.len() > 4 {
                plugin_analyzer::is_binary(&data[..])
                    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
            }
        }
        let data = data.freeze();

        storage.upload(&data[..]).await.unwrap();
    }

    Ok(())
}
