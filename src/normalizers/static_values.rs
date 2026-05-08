use crate::models::normalization::Normalization;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static NORMALIZATION: LazyLock<Normalization> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../resources/normalization.json"))
        .expect("invalid resources/normalization.json")
});

pub static MCC_RISK: LazyLock<HashMap<String, f32>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../resources/mcc_risk.json"))
        .expect("invalid resources/mcc_risk.json")
});
