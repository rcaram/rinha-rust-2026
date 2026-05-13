use crate::models::fraud_score_request::FraudScoreRequest;
use crate::models::fraud_score_response::FraudScoreResponse;
use crate::{AppState, normalizers::normalize_data::normalize_data};

use axum::{Json, extract::State};
use std::sync::Arc;
use tokio::task;

const K_NEIGHBORS: usize = 5;
const APPROVAL_THRESHOLD: f32 = 0.6;

pub async fn create_fraud_score(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FraudScoreRequest>,
) -> Json<FraudScoreResponse> {
    let normalized_data = normalize_data(payload);
    let permit = match state.scoring_semaphore.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            return Json(FraudScoreResponse {
                approved: false,
                fraud_score: 1.0,
            });
        }
    };
    let vector_store = state.vector_store.clone();
    let fraud_hits = match task::spawn_blocking(move || {
        vector_store.query_fraud_hits(&normalized_data, K_NEIGHBORS)
    })
    .await
    {
        Ok(Ok(hits)) => hits,
        _ => {
            drop(permit);
            return Json(FraudScoreResponse {
                approved: false,
                fraud_score: 1.0,
            });
        }
    };

    drop(permit);

    let score = fraud_hits as f32 / K_NEIGHBORS as f32;
    Json(FraudScoreResponse {
        approved: score < APPROVAL_THRESHOLD,
        fraud_score: score,
    })
}
