use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use std::env;
use std::sync::Arc;
use tokio::sync::Semaphore;
mod database;
mod handlers;
mod models;
mod normalizers;

use database::vectordb::get_store;
use handlers::fraud_score_handler::create_fraud_score;

#[derive(Clone)]
pub struct AppState {
    pub vector_store: Arc<database::vectordb::VectorStore>,
    pub scoring_semaphore: Arc<Semaphore>,
}

#[tokio::main] // tells tokio to run this as async
async fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let bind_address = format!("0.0.0.0:{port}");
    let max_in_flight = env::var("SCORING_MAX_IN_FLIGHT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4);
    let vector_store = get_store()
        .unwrap_or_else(|error| panic!("failed to load vector store during startup: {error}"))
        .clone();
    let app_state = Arc::new(AppState {
        vector_store,
        scoring_semaphore: Arc::new(Semaphore::new(max_in_flight)),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/fraud-score", post(create_fraud_score))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_address).await.unwrap();

    println!("Listening on http://localhost:{port}");

    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "OK"
}

async fn ready(State(state): State<Arc<AppState>>) -> StatusCode {
    if state.vector_store.is_empty() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    StatusCode::OK
}
