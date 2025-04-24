use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use bytes::BytesMut;
use log::info;
use memflow::plugins::plugin_analyzer;
use tokio_util::io::ReaderStream;

use crate::{
    error::ResponseResult,
    storage::{database::PluginDatabaseFindParams, PluginMetadata, Storage, UploadResponse},
};

use super::{
    middlewares::{check_token, AuthorizationToken},
    models::{PluginUploadResponse, PluginsAllResponse, PluginsFindResponse},
};

pub fn app(storage: Storage, auth_token: AuthorizationToken) -> Router {
    let authed_routes = Router::new()
        .route("/files", post(upload_file))
        .route("/files/{digest}", delete(delete_file_by_digest))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)) // 20 mb
        .route_layer(middleware::from_fn_with_state(
            auth_token.clone(),
            check_token,
        ))
        .with_state(storage.clone());

    let public_routes = Router::new()
        .route("/plugins", get(get_plugins))
        .route("/plugins/{plugin_name}", get(find_plugin_variants))
        .route("/files/{digest}", get(download_file_by_digest))
        .route("/files/{digest}/metadata", get(get_file_metadata_by_digest))
        .with_state(storage);

    Router::new().merge(public_routes).merge(authed_routes)
}

/// Returns a list of all available plugins
async fn get_plugins(State(storage): State<Storage>) -> ResponseResult<Json<PluginsAllResponse>> {
    let plugins = storage.database().plugins();
    Ok(PluginsAllResponse { plugins }.into())
}

/// Returns a list of plugins based on the given filter parameters
async fn find_plugin_variants(
    State(storage): State<Storage>,
    params: Query<PluginDatabaseFindParams>,
    Path(plugin_name): Path<String>,
) -> ResponseResult<Json<PluginsFindResponse>> {
    // find entries in database
    let params: PluginDatabaseFindParams = params.0;
    let entries = storage
        .database()
        .plugin_variants(&plugin_name, params.clone());

    Ok(PluginsFindResponse {
        plugins: entries,
        skip: params.skip.unwrap_or(0),
    }
    .into())
}

/// Posts a file to the backend and analyzes it.
async fn upload_file(
    State(storage): State<Storage>,
    mut multipart: Multipart,
) -> ResponseResult<Json<PluginUploadResponse>> {
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

            // TODO: do not require duplicate struct definitions here
            // upload file
            let result = storage.upload(&data[..], &signature).await;
            match result {
                Ok(UploadResponse::Added) => Ok(PluginUploadResponse::Added.into()),
                Ok(UploadResponse::AlreadyExists) => Ok(PluginUploadResponse::AlreadyExists.into()),
                Err(err) => Err((StatusCode::BAD_REQUEST, err.to_string())),
            }
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

/// Retrieves a file by it's digest.
async fn download_file_by_digest(
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

/// Retrieves a file's metadata by it's digest.
async fn get_file_metadata_by_digest(
    State(storage): State<Storage>,
    Path(digest): Path<String>,
) -> ResponseResult<Json<PluginMetadata>> {
    // try to download the file by its digest
    let metadata = storage
        .metadata(&digest)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "plugin not found".to_owned()))?;
    Ok(metadata.into())
}

/// Deletes the file with the given digest.
async fn delete_file_by_digest(
    State(storage): State<Storage>,
    Path(digest): Path<String>,
) -> ResponseResult<()> {
    // try to delete the file by its digest
    storage
        .delete(&digest)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(())
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
        //let app = app(storage);

        // run tests
    }
}
