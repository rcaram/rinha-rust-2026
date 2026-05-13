use crate::database::vectordb::load_store_from_path;
use std::error::Error;

pub fn load_file(file_path: &str) -> Result<(), Box<dyn Error>> {
    let _ = load_store_from_path(file_path)?;
    Ok(())
}
