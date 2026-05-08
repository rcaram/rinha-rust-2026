#[path = "src/database/vectordb.rs"]
mod vectordb_impl;

#[path = "src/models/fraud_load_vector.rs"]
mod fraud_load_vector_impl;

#[path = "src/handlers/load_score_handler.rs"]
mod load_score_handler;

mod database {
    pub mod vectordb {
        pub use crate::vectordb_impl::*;
    }
}

mod models {
    pub mod fraud_load_vector {
        pub use crate::fraud_load_vector_impl::*;
    }
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let vectors_file = std::env::var("LOAD_SCORE_FILE")
        .unwrap_or_else(|_| format!("{manifest_dir}/resources/references.json.gz"));
    let db_path = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("{manifest_dir}/resources/vectors.db"));

    println!("cargo:rerun-if-changed={vectors_file}");
    println!("cargo:rerun-if-env-changed=LOAD_SCORE_FILE");
    println!("cargo:rerun-if-env-changed=DATABASE_URL");

    if !std::path::Path::new(&vectors_file).exists() {
        println!("cargo:warning=vector source file not found: {vectors_file}");
        return;
    }

    unsafe {
        std::env::set_var("DATABASE_URL", &db_path);
    }

    if let Err(err) = load_score_handler::load_file_to_db(&vectors_file) {
        panic!("failed loading vectors during build: {err}");
    }
}
