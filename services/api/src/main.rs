use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: String,
}

#[derive(Clone)]
struct AppState {
    service_name: String,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: state.service_name,
    })
}

async fn shutdown_handler() {
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
    let terminate = async { std::future::pending::<()>().await; };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutdown signal received");
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bricks_api=debug,axum=info,tower_http=debug".into()),
        )
        .init();

    let state = AppState {
        service_name: "bricks-api".to_string(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("listening on {}", addr);

    let serve = axum::serve(
        tokio::net::TcpListener::bind(&addr)
            .await
            .expect("failed to bind to address"),
        app,
    );

    serve
        .with_graceful_shutdown(shutdown_handler())
        .await
        .expect("server error");
}
