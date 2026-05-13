use flate2::read::GzDecoder;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

const VECTOR_DIM: usize = 14;
const BINARY_MAGIC: u32 = 0x5645_4352;
const BINARY_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Label {
    Legit,
    Fraud,
}

#[derive(Deserialize)]
struct FraudLoadVector {
    vector: Vec<f32>,
    label: Label,
}

fn output_path_for_source(source: &str) -> String {
    if let Ok(configured) = std::env::var("LOAD_SCORE_BINARY") {
        return configured;
    }

    if let Some(stripped) = source.strip_suffix(".json.gz") {
        return format!("{stripped}.bin");
    }
    if let Some(stripped) = source.strip_suffix(".gz") {
        return format!("{stripped}.bin");
    }
    if let Some(stripped) = source.strip_suffix(".json") {
        return format!("{stripped}.bin");
    }
    format!("{source}.bin")
}

fn open_reader(path: &str) -> Result<Box<dyn std::io::Read>, std::io::Error> {
    let file = File::open(path)?;
    if path.ends_with(".gz") {
        Ok(Box::new(BufReader::new(GzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let source_path = std::env::var("LOAD_SCORE_FILE")
        .unwrap_or_else(|_| format!("{manifest_dir}/resources/references.json.gz"));
    let output_path = output_path_for_source(&source_path);

    println!("cargo:rerun-if-changed={source_path}");
    println!("cargo:rerun-if-env-changed=LOAD_SCORE_FILE");
    println!("cargo:rerun-if-env-changed=LOAD_SCORE_BINARY");

    if !Path::new(&source_path).exists() {
        println!("cargo:warning=vector source file not found: {source_path}");
        return;
    }

    let reader = match open_reader(&source_path) {
        Ok(reader) => reader,
        Err(err) => panic!("failed opening vectors source at {source_path}: {err}"),
    };

    let rows: Vec<FraudLoadVector> = match serde_json::from_reader(reader) {
        Ok(rows) => rows,
        Err(err) => panic!("failed parsing vectors from {source_path}: {err}"),
    };

    let count = rows.len();
    let mut vectors = Vec::with_capacity(count * VECTOR_DIM);
    let mut labels = Vec::with_capacity(count);

    for (index, row) in rows.into_iter().enumerate() {
        if row.vector.len() != VECTOR_DIM {
            panic!(
                "invalid vector size at row {index}: expected {VECTOR_DIM}, got {}",
                row.vector.len()
            );
        }

        vectors.extend_from_slice(&row.vector);
        labels.push(matches!(row.label, Label::Fraud) as u8);
    }

    let count_u32 = u32::try_from(count).unwrap_or_else(|_| {
        panic!("vector rows exceed u32 limit: {count}");
    });

    let output = match File::create(&output_path) {
        Ok(file) => file,
        Err(err) => panic!("failed creating binary vectors file at {output_path}: {err}"),
    };
    let mut writer = BufWriter::new(output);

    if let Err(err) = writer.write_all(&BINARY_MAGIC.to_le_bytes()) {
        panic!("failed writing binary header: {err}");
    }
    if let Err(err) = writer.write_all(&BINARY_VERSION.to_le_bytes()) {
        panic!("failed writing binary version: {err}");
    }
    if let Err(err) = writer.write_all(&(VECTOR_DIM as u32).to_le_bytes()) {
        panic!("failed writing binary dimension: {err}");
    }
    if let Err(err) = writer.write_all(&count_u32.to_le_bytes()) {
        panic!("failed writing binary count: {err}");
    }

    for value in vectors {
        if let Err(err) = writer.write_all(&value.to_le_bytes()) {
            panic!("failed writing vectors payload: {err}");
        }
    }
    if let Err(err) = writer.write_all(&labels) {
        panic!("failed writing labels payload: {err}");
    }
    if let Err(err) = writer.flush() {
        panic!("failed flushing binary vectors file: {err}");
    }
}
