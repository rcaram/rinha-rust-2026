use serde::Deserialize;

#[derive(Deserialize)]
pub struct Normalization {
    pub max_amount: f32,
    pub max_installments: u32,
    pub amount_vs_avg_ratio: f32,
    pub max_minutes: u32,
    pub max_km: f32,
    pub max_tx_count_24h: u32,
    pub max_merchant_avg_amount: f32,
}
