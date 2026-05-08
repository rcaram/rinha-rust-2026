use crate::database::vectordb::{create_table, drop_table, insert_vectors, open_db};
use crate::models::fraud_load_vector::{FraudLoadVector, Label};
use flate2::read::GzDecoder;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Error as IoError, ErrorKind};

pub fn load_file_to_db(file_path: &str) -> Result<(), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let gz = GzDecoder::new(file);
    let reader = BufReader::new(gz);
    let rows: Vec<FraudLoadVector> = serde_json::from_reader(reader)?;
    if rows.is_empty() {
        return Ok(());
    }
    if rows.iter().any(|row| row.vector.len() != 14) {
        return Err(Box::new(IoError::new(
            ErrorKind::InvalidData,
            "invalid vector length: expected 14",
        )));
    }
    let (vectors, labels): (Vec<Vec<f32>>, Vec<bool>) = rows
        .into_iter()
        .map(|row| (row.vector, matches!(row.label, Label::Fraud)))
        .unzip();
    let db = open_db(false)?;
    drop_table(&db)?;
    create_table(&db)?;
    insert_vectors(&db, &vectors, &labels)?;
    Ok(())
}
