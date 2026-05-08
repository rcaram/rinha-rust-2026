use crate::models::fraud_score_request::FraudScoreRequest;
use crate::models::fraud_score_response::FraudScoreResponse;
use crate::{
    database::vectordb::{open_db, query_vectors},
    normalizers::normalize_data::normalize_data,
};
use rusqlite::Connection;
use std::{cell::RefCell, thread_local};

use axum::Json;
use tokio::task;

const K_NEIGHBORS: usize = 5;
const APPROVAL_THRESHOLD: f32 = 0.6;

thread_local! {
    static DB: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

fn with_db<F, T>(operation: F) -> rusqlite::Result<T>
where
    F: FnOnce(&Connection) -> rusqlite::Result<T>,
{
    DB.with(|db| {
        let mut db_ref = db.borrow_mut();
        if db_ref.is_none() {
            *db_ref = Some(open_db(true)?);
        }

        if let Some(conn) = db_ref.as_ref() {
            operation(conn)
        } else {
            Err(rusqlite::Error::InvalidQuery)
        }
    })
}

pub async fn create_fraud_score(
    Json(payload): Json<FraudScoreRequest>,
) -> Json<FraudScoreResponse> {
    let normalized_data = normalize_data(payload);
    let fraud_hits = match task::spawn_blocking(move || {
        with_db(|db| query_vectors(db, &normalized_data, K_NEIGHBORS))
    })
    .await
    {
        Ok(Ok(fraud_hits)) => fraud_hits,
        Ok(Err(_)) | Err(_) => {
            return Json(FraudScoreResponse {
                approved: false,
                fraud_score: 1.0,
            });
        }
    };

    let score = fraud_hits as f32 / K_NEIGHBORS as f32;
    Json(FraudScoreResponse {
        approved: score < APPROVAL_THRESHOLD,
        fraud_score: score,
    })
}
