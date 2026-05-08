use axum::{
    Router,
    routing::{get, post},
};
use std::env;
mod database;
mod handlers;
mod models;
mod normalizers;

use handlers::fraud_score_handler::create_fraud_score;

#[tokio::main] // tells tokio to run this as async
async fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let bind_address = format!("0.0.0.0:{port}");

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/fraud-score", post(create_fraud_score));

    let listener = tokio::net::TcpListener::bind(&bind_address).await.unwrap();

    println!("Listening on http://localhost:{port}");

    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "OK"
}

async fn ready() -> &'static str {
    "OK"
}
