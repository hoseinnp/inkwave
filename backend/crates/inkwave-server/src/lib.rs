use axum::{routing::get, Json, Router};
use inkwave_core::HealthStatus;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn health() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "ok".to_string(),
        version: VERSION.to_string(),
    })
}

pub fn app() -> Router {
    Router::new().route("/health", get(health))
}
