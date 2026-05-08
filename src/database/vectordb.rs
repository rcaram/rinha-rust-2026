use rusqlite::{Connection, Result, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;
use zerocopy::IntoBytes;

pub fn open_db(readonly: bool) -> Result<Connection> {
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let db_path =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "/app/resources/vectors.db".to_string());

    let conn = Connection::open(&db_path)?;

    if readonly {
        conn.pragma_update(None, "journal_mode", "OFF")?;
        conn.pragma_update(None, "locking_mode", "EXCLUSIVE")?;
        conn.pragma_update(None, "mmap_size", "268435456")?;
        conn.pragma_update(None, "cache_size", "-65536")?;
        conn.pragma_update(None, "query_only", "ON")?;
    } else {
        conn.pragma_update(None, "journal_mode", "WAL")?;
    }
    conn.pragma_update(None, "synchronous", "OFF")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;

    Ok(conn)
}

pub fn create_table(db: &Connection) -> Result<()> {
    db.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS fraud_vectors USING vec0(embeddings float[14], label INT)",
        [],
    )?;
    Ok(())
}

pub fn insert_vectors(db: &Connection, vectors: &[Vec<f32>], labels: &[bool]) -> Result<()> {
    let mut stmt = db.prepare("INSERT INTO fraud_vectors (embeddings, label) VALUES (?, ?)")?;

    for (vector, label) in vectors.iter().zip(labels.iter()) {
        stmt.execute(params![
            vector.as_bytes(),
            if *label { 1_i64 } else { 0_i64 }
        ])?;
    }

    Ok(())
}

pub fn drop_table(db: &Connection) -> Result<()> {
    db.execute("DROP TABLE IF EXISTS fraud_vectors", [])?;
    Ok(())
}

#[allow(dead_code)]
pub fn query_vectors(db: &Connection, query: &[f32], k: usize) -> Result<i8> {
    let mut stmt = db.prepare_cached(
        r#"
        SELECT COALESCE(SUM(label), 0)
        FROM (
            SELECT label
            FROM fraud_vectors
            WHERE embeddings MATCH ?
            ORDER BY distance
            LIMIT ?
        )
        "#,
    )?;

    stmt.query_row(params![query.as_bytes(), k as i64], |row| row.get(0))
}
