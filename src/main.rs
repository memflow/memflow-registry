use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use bytes::BytesMut;
use error::ResponseResult;
use log::info;
use tokio_util::io::ReaderStream;

mod error;
mod storage;
use serde::Serialize;
use storage::{
    plugin_analyzer::{self},
    PluginDatabaseFindParams, PluginEntry, Storage,
};

#[tokio::main]
async fn main() {
    env_logger::init();

    let store = Storage::new();

    // build our application with a single route
    let app = Router::new()
        .route("/", post(plugin_push))
        .route("/find", get(plugin_find))
        .route("/:digest", get(plugin_pull))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)) // 20 mb
        .with_state(store);

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
        // read the buffer
        let mut data = BytesMut::new();
        let mut checked = false;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
        {
            data.extend_from_slice(&chunk);

            // check if this file is a potential binary or early abort
            if !checked && data.len() > 4 {
                plugin_analyzer::is_binary(&data[..])
                    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
                checked = true;
            }
        }
        let data = data.freeze();

        storage
            .upload(&data[..])
            .await
            .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    }

    Ok(())
}

#[derive(Clone, Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginEntry>,
    skip: usize,
}

async fn plugin_find(
    State(storage): State<Storage>,
    params: Query<PluginDatabaseFindParams>,
) -> ResponseResult<Json<PluginListResponse>> {
    let params: PluginDatabaseFindParams = params.0;

    // find entries in database
    let entries = storage.database().find(params.clone());

    Ok(PluginListResponse {
        plugins: entries,
        skip: params.skip.unwrap_or(0),
    }
    .into())
}

async fn plugin_pull(
    State(storage): State<Storage>,
    Path(digest): Path<String>,
) -> ResponseResult<impl IntoResponse> {
    // try to download the file by its digest
    let file = storage
        .download(&digest)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "plugin not found".to_owned()))?;

    // convert into a stream
    let file_len = file.metadata().await.unwrap().len();
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut response = body.into_response();

    let headers = response.headers_mut();
    headers.append(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers.append(
        CONTENT_LENGTH,
        HeaderValue::from_str(&format!("{}", file_len)).unwrap(),
    );

    Ok(response)
}
