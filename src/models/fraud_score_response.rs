use serde::Serialize;

#[derive(Serialize)]
pub struct FraudScoreResponse {
    pub approved: bool,
    pub fraud_score: f32,
}
