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
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use bytes::BytesMut;
use log::{info, warn};
use serde::Serialize;
use tokio_util::io::ReaderStream;

mod error;
mod storage;

use error::ResponseResult;
use storage::{
    database::{PluginDatabaseFindParams, PluginEntry},
    pki::SignatureVerifier,
    plugin_analyzer, Storage,
};

#[tokio::main]
async fn main() {
    // load configuration from .env file
    dotenv::dotenv().ok();

    // initialize logging
    env_logger::init();

    match std::env::var("MEMFLOW_BEARER_TOKEN") {
        Ok(token) if token.is_empty() => {
            warn!("authentication token is empty, THIS IS POTENTIALLY INSECURE.")
        }
        Err(_) => warn!("no authentication token set, THIS IS POTENTIALLY INSECURE."),
        _ => (),
    }

    let root = std::env::var("MEMFLOW_STORAGE_ROOT").unwrap_or_else(|_| ".storage".into());
    info!("storing plugins in `{}`", root);
    let mut storage = Storage::new(&root).expect("unable to create storage handler");

    // use public key file if specified
    if let Ok(public_key_file) = std::env::var("MEMFLOW_PUBLIC_KEY_FILE") {
        let signature_verifier =
            SignatureVerifier::new(public_key_file).expect("unable to load public key file");
        storage = storage.with_signature_verifier(signature_verifier);
    } else {
        warn!("public key file not set, THIS IS POTENTIALLY INSECURE.");
    }

    // build our application with a single route
    let app = app(storage);

    // run our app with hyper, listening globally on port 3000
    let addr = std::env::var("MEMFLOW_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("serving memflow-registry on `{}`", addr);
    axum::serve(listener, app).await.unwrap();
}

fn app(storage: Storage) -> Router {
    Router::new()
        .route("/", post(plugin_push))
        .route("/find", get(plugin_find))
        .route("/:digest", get(plugin_pull))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)) // 20 mb
        .with_state(storage)
}

async fn plugin_push(
    State(storage): State<Storage>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    mut multipart: Multipart,
) -> ResponseResult<()> {
    // TODO: move to state?
    if let Ok(token) = std::env::var("MEMFLOW_BEARER_TOKEN") {
        if authorization.0.token() != token {
            warn!(
                "invalid token when uploading plugin: token={}",
                authorization.0.token()
            );
            return Err((StatusCode::FORBIDDEN, "invalid token".to_owned()));
        }
    }

    let mut file_data = None;
    let mut file_signature = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?
    {
        if let Some(name) = field.name() {
            match name {
                "signature" => {
                    file_signature = Some(
                        field
                            .text()
                            .await
                            .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?,
                    );
                }
                "file" => {
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
                    file_data = Some(data.freeze());
                }
                _ => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "unexpected field in multipart form".to_owned(),
                    ))
                }
            }
        }
    }

    if let Some(data) = file_data {
        if let Some(signature) = file_signature {
            info!(
                "trying to add file to registry: size={} signature={}",
                data.len(),
                &signature
            );

            // upload file
            storage
                .upload(&data[..], &signature)
                .await
                .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
            Ok(())
        } else {
            Err((
                StatusCode::BAD_REQUEST,
                "file signature is required".to_owned(),
            ))
        }
    } else {
        Err((StatusCode::BAD_REQUEST, "file is required".to_owned()))
    }
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

#[cfg(test)]
mod test {
    use axum::http::Request;
    use tower::util::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn push() {
        // create temporary directory
        let root = tempfile::tempdir().unwrap();
        let storage = Storage::new(root.into_path()).expect("unable to create storage handler");
        let app = app(storage);

        // run tests
    }
}
