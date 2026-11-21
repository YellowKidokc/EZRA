use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

const APPLICATION_ID: i64 = 0x455A5241; // EZRA

pub fn open(path: &Path) -> Result<Connection> {
    let path = crate::platform::local_path(path)?;
    let parent = path
        .parent()
        .context("Index path needs a parent directory")?;
    fs::create_dir_all(parent)?;
    if !path.try_exists()? {
        // Publish only a fully initialized index. Concurrent first runs can safely race.
        let temp = tempfile::NamedTempFile::new_in(parent)?;
        let mut db = Connection::open(temp.path())?;
        let tx = db.transaction()?;
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.commit()?;
        drop(db);
        match temp.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    let db = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("Open local index {}", path.display()))?;
    db.busy_timeout(Duration::from_secs(30))?;
    let id: i64 = db.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if id != APPLICATION_ID {
        bail!(
            "Refusing to write an existing file that is not an EZRA index: {}",
            path.display()
        );
    }
    db.pragma_update(None, "foreign_keys", true)?;
    Ok(db)
}

#[derive(Clone, Debug)]
pub struct Location {
    pub row: i64,
    pub record: Option<String>,
    pub path: String,
    pub hash: Option<String>,
    pub size: Option<i64>,
    pub mtime: Option<String>,
    pub first: i64,
    pub revision: i64,
    pub status: String,
    pub kind: String,
    pub claimed: Option<String>,
}

const LOCATION_COLUMNS: &str =
    "id,record_id,path,sha256,size,mtime,first_seen,revision_seq,status,kind,claimed_id";

fn read_location(row: &rusqlite::Row<'_>) -> rusqlite::Result<Location> {
    Ok(Location {
        row: row.get(0)?,
        record: row.get(1)?,
        path: row.get(2)?,
        hash: row.get(3)?,
        size: row.get(4)?,
        mtime: row.get(5)?,
        first: row.get(6)?,
        revision: row.get(7)?,
        status: row.get(8)?,
        kind: row.get(9)?,
        claimed: row.get(10)?,
    })
}

pub fn locations(db: &Connection) -> Result<Vec<Location>> {
    let mut query = db.prepare(&format!(
        "SELECT {LOCATION_COLUMNS} FROM locations WHERE is_current=1 ORDER BY path"
    ))?;
    Ok(query
        .query_map([], read_location)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn lookup_key(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

pub fn alias(db: &Connection, id: &str, name: &str, source: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        bail!("Alias must be nonempty and contain no control characters");
    }
    db.execute(
        "INSERT OR IGNORE INTO aliases(record_id,alias,normalized_lookup_key,source) VALUES (?,?,?,?)",
        params![id, name, lookup_key(name), source],
    )?;
    Ok(())
}

pub fn lookup(db: &Connection, value: &str) -> Result<Vec<String>> {
    if let Ok(id) = uuid::Uuid::parse_str(value) {
        let exact: Option<String> = db
            .query_row(
                "SELECT id FROM records WHERE id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(exact) = exact {
            return Ok(vec![exact]);
        }
    }
    let mut query = db.prepare(
        "SELECT DISTINCT record_id FROM aliases WHERE normalized_lookup_key = ? ORDER BY record_id",
    )?;
    Ok(query
        .query_map([lookup_key(value)], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn locations_at(db: &Connection, path: &str) -> Result<Option<Location>> {
    Ok(db
        .query_row(
            &format!("SELECT {LOCATION_COLUMNS} FROM locations WHERE is_current=1 AND path=?"),
            [path],
            read_location,
        )
        .optional()?)
}
