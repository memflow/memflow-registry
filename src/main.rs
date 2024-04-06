use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use log::{info, warn};
use rest::middlewares::AuthorizationToken;
use serde::{Deserialize, Serialize};
use tokio::signal;

mod default_registry;
mod error;
mod pki;
mod plugin_uri;
mod rest;
mod storage;

use pki::SignatureVerifier;
use storage::Storage;

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
    let auth_token = AuthorizationToken::new(std::env::var("MEMFLOW_BEARER_TOKEN").ok());

    let routes = Router::new()
        .route("/health", get(health))
        .with_state(storage.clone());

    Router::new()
        .merge(routes)
        .merge(rest::routes::app(storage, auth_token))
}

/// Health status of the service
#[derive(Debug, Serialize, Deserialize)]
pub enum HealthResponse {
    Ok,
    Error(String),
}

/// Returns a health status
async fn health(
    State(storage): State<Storage>,
) -> std::result::Result<Json<HealthResponse>, (axum::http::StatusCode, Json<HealthResponse>)> {
    match storage.health() {
        Ok(()) => Ok(HealthResponse::Ok.into()),
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            HealthResponse::Error("database unhealthy".to_owned()).into(),
        )),
    }
}
