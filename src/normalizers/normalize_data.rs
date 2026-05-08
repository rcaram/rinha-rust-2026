use crate::models::fraud_score_request::FraudScoreRequest;
use crate::normalizers::static_values::{MCC_RISK, NORMALIZATION};
use chrono::{Datelike, Timelike};

pub fn normalize_data(request: FraudScoreRequest) -> Vec<f32> {
    let clamp01 = |v: f32| v.clamp(0.0, 1.0);
    let amount = clamp01(request.transaction.amount / NORMALIZATION.max_amount);
    let installments =
        clamp01(request.transaction.installments as f32 / NORMALIZATION.max_installments as f32);
    let amount_vs_avg = if request.customer.avg_amount > 0.0 {
        clamp01(
            (request.transaction.amount / request.customer.avg_amount)
                / NORMALIZATION.amount_vs_avg_ratio,
        )
    } else {
        0.0
    };
    let hour_of_day = request.transaction.requested_at.hour() as f32 / 23.0;
    let day_of_week = request
        .transaction
        .requested_at
        .weekday()
        .num_days_from_monday() as f32
        / 6.0;
    let (minutes_since_last_tx, km_from_last_tx) = match request.last_transaction {
        Some(last_tx) => {
            let minutes = request
                .transaction
                .requested_at
                .signed_duration_since(last_tx.timestamp)
                .num_minutes() as f32;
            (
                clamp01(minutes / NORMALIZATION.max_minutes as f32),
                clamp01(last_tx.km_from_current / NORMALIZATION.max_km),
            )
        }
        None => (-1.0, -1.0),
    };
    let km_from_home = clamp01(request.terminal.km_from_home / NORMALIZATION.max_km);
    let tx_count_24h =
        clamp01(request.customer.tx_count_24h as f32 / NORMALIZATION.max_tx_count_24h as f32);
    let is_online = if request.terminal.is_online { 1.0 } else { 0.0 };
    let card_present = if request.terminal.card_present {
        1.0
    } else {
        0.0
    };
    let unknown_merchant = if request
        .customer
        .known_merchants
        .iter()
        .any(|known| known == &request.merchant.id)
    {
        0.0
    } else {
        1.0
    };
    let mcc_risk = MCC_RISK.get(&request.merchant.mcc).copied().unwrap_or(0.5);
    let merchant_avg_amount =
        clamp01(request.merchant.avg_amount / NORMALIZATION.max_merchant_avg_amount);
    vec![
        amount,                // 0
        installments,          // 1
        amount_vs_avg,         // 2
        hour_of_day,           // 3
        day_of_week,           // 4
        minutes_since_last_tx, // 5
        km_from_last_tx,       // 6
        km_from_home,          // 7
        tx_count_24h,          // 8
        is_online,             // 9
        card_present,          // 10
        unknown_merchant,      // 11
        mcc_risk,              // 12
        merchant_avg_amount,   // 13
    ]
}
