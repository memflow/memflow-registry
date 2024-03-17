use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use bytes::BytesMut;
use error::ResponseResult;
use log::info;
use tokio_util::io::ReaderStream;

mod error;
mod storage;
use serde::Deserialize;
use storage::{
    plugin_analyzer::{self, PluginArchitecture, PluginFileType},
    Storage,
};

#[tokio::main]
async fn main() {
    env_logger::init();

    let store = Storage::new();

    // build our application with a single route
    let app = Router::new()
        .route("/", post(plugin_push))
        .route("/", get(plugin_pull))
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

#[derive(Debug, Deserialize)]
struct PluginPullParams {
    plugin_name: String,
    plugin_version: i32,
    file_type: PluginFileType,
    architecture: PluginArchitecture,
    tag: Option<String>,
}

async fn plugin_pull(
    State(storage): State<Storage>,
    params: Query<PluginPullParams>,
) -> ResponseResult<impl IntoResponse> {
    let params: PluginPullParams = params.0;

    // try to open the file
    let file = storage
        .download(
            &params.plugin_name,
            params.plugin_version,
            &params.file_type,
            &params.architecture,
            params.tag.as_ref().map(String::as_ref).unwrap_or("latest"),
        )
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
