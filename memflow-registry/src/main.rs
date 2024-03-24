use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use bytes::BytesMut;
use log::{info, warn};
use tokio::signal;
use tokio_util::io::ReaderStream;

use memflow_registry_shared::{
    plugin_analyzer, structs::PluginsFindResponse, PluginsAllResponse, ResponseResult,
    SignatureVerifier,
};

mod storage;
use storage::{database::PluginDatabaseFindParams, Storage};

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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

fn app(storage: Storage) -> Router {
    let auth_token = AuthorizationToken {
        token: std::env::var("MEMFLOW_BEARER_TOKEN").ok(),
    };
    let authed_routes = Router::new()
        .route("/files", post(upload_file))
        .route("/files/:digest", delete(delete_file_by_digest))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)) // 20 mb
        .route_layer(middleware::from_fn_with_state(
            auth_token.clone(),
            check_token,
        ))
        .with_state(storage.clone());

    let public_routes = Router::new()
        .route("/plugins", get(get_plugins))
        .route("/plugins/:plugin_name", get(find_plugin_variants))
        .route("/files/:digest", get(find_file_by_digest))
        .with_state(storage);

    Router::new().merge(public_routes).merge(authed_routes)
}

#[derive(Clone)]
struct AuthorizationToken {
    token: Option<String>,
}

async fn check_token(
    State(auth_token): State<AuthorizationToken>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if let Some(token) = auth_token.token {
        if authorization.0.token() != token {
            // token is set but it does not match
            warn!(
                "invalid token when uploading plugin: token={}",
                authorization.0.token()
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let response = next.run(request).await;
    Ok(response)
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
) -> ResponseResult<()> {
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

/// Retrieves a file by it's digest.
async fn find_file_by_digest(
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

/// Deletes the file with the given digest.
async fn delete_file_by_digest(
    State(storage): State<Storage>,
    Path(digest): Path<String>,
) -> ResponseResult<()> {
    // try to delete the file by its digest
    storage.delete(&digest).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "digest could not be deleted".to_owned(),
        )
    })?;

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
        let app = app(storage);

        // run tests
    }
}
