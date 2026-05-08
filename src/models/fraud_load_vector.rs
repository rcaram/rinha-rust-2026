use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    Legit,
    Fraud,
}

#[derive(Deserialize)]
pub struct FraudLoadVector {
    pub vector: Vec<f32>,
    pub label: Label,
}
