use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::config;

fn connect() -> crate::Result<Connection> {
    let directory = config::directory();
    let path = &directory.join("data.db");
    let conn = Connection::open(path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS history (
            timestamp INTEGER,
            input TEXT,
            path TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_timestamp_input
        ON history (
            timestamp,
            input
        )",
        [],
    )?;

    Ok(conn)
}

pub fn insert(input: &Path, path: &Path) -> crate::Result<()> {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH)?;
    let timestamp = duration.as_secs();

    let conn = connect()?;
    conn.execute(
        "INSERT INTO history (timestamp, input, path) VALUES (?1, ?2, ?3)",
        params![timestamp, input.to_str().unwrap(), path.to_str().unwrap()],
    )?;
    Ok(())
}

pub fn find(input: &Path) -> crate::Result<Option<PathBuf>> {
    let conn = connect()?;
    let result = conn.query_row(
        "SELECT path FROM history
        WHERE input = ?1
        ORDER BY timestamp DESC
        LIMIT 1",
        params![input.to_str().unwrap()],
        |row| row.get(0),
    );

    let path: Option<String> = result.optional()?;
    Ok(path.map(PathBuf::from))
}
