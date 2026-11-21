use crate::index;
use anyhow::{Context, Result, bail};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use rusqlite::{Connection, params};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Folder,
    Map,
    Info,
}

pub fn link(value: &str) -> String {
    let encoded = utf8_percent_encode(value, NON_ALPHANUMERIC).to_string();
    format!(
        "tp://{}",
        if uuid::Uuid::parse_str(value).is_ok() {
            value
        } else {
            &encoded
        }
    )
}

pub fn parse(uri: &str) -> Result<(String, Action)> {
    let (scheme, target) = uri.split_once("://").context("Expected a tp:// link")?;
    if !scheme.eq_ignore_ascii_case("tp") || target.contains('#') {
        bail!("Invalid tp:// link");
    }
    let (value, query) = target.split_once('?').unwrap_or((target, ""));
    let value = value.strip_suffix('/').unwrap_or(value);
    if value.contains(['/', '\\', '@', ':']) || value.is_empty() {
        bail!("Invalid tp:// target");
    }
    let bytes = value.as_bytes();
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && !bytes
                .get(offset + 1..offset + 3)
                .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
        {
            bail!("Malformed percent-encoding");
        }
    }
    let name = percent_decode_str(value).decode_utf8()?.into_owned();
    if name.chars().any(char::is_control) {
        bail!("Invalid control character in tp:// target");
    }
    let action = match query {
        "" => Action::Open,
        "folder" | "folder=" => Action::Folder,
        "map" | "map=" => Action::Map,
        "info" | "info=" => Action::Info,
        _ => bail!("Unsupported tp:// query"),
    };
    Ok((name, action))
}

pub enum Resolution {
    Path(PathBuf, bool),
    Card(String),
}

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn resolve(db: &Connection, uri: &str) -> Result<Resolution> {
    let (name, action) = parse(uri)?;
    let mut ids: BTreeSet<String> = index::lookup(db, &name)?.into_iter().collect();
    // A disputed claim remains visible even when it never established an indexed record.
    let claim = uuid::Uuid::parse_str(&name).ok().map(|id| id.to_string());
    if let Some(claim) = &claim {
        let disputed: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM locations WHERE claimed_id=? AND is_current=1 AND status='conflict')",
            [claim], |row| row.get(0),
        )?;
        if disputed {
            ids.insert(claim.clone());
        }
    }
    if ids.is_empty() {
        bail!("No record found for {name}");
    }
    let mut query = db.prepare(
        "SELECT l.path,l.status,l.last_seen,l.revision_seq,l.sha256,l.reason,
                COALESCE(r.title, l.claimed_id),l.missing_since
         FROM locations l LEFT JOIN records r ON l.record_id=r.id
         WHERE l.is_current=1 AND (l.record_id=?1 OR (l.claimed_id=?1 AND l.status='conflict'))
         ORDER BY l.path",
    )?;
    let mut active = Vec::new();
    let mut conflict = false;
    let mut card = String::new();
    for id in &ids {
        let observations = query.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<i64>>(7)?,
            ))
        })?;
        for row in observations {
            let (path, mut status, last_seen, rev, hash, reason, title, missing_since) = row?;
            let on_disk = Path::new(&path).exists();
            if status == "active" {
                if on_disk {
                    active.push(PathBuf::from(&path));
                } else {
                    status = "unknown".to_owned();
                }
            }
            conflict |= status == "conflict";
            let date: String =
                db.query_row("SELECT datetime(? / 1000, 'unixepoch')", [last_seen], |r| {
                    r.get(0)
                })?;
            let timing = if status == "missing"
                && let Some(since) = missing_since
            {
                let since: String =
                    db.query_row("SELECT datetime(? / 1000, 'unixepoch')", [since], |row| {
                        row.get(0)
                    })?;
                format!("missing since {since} UTC · last seen {date} UTC")
            } else {
                format!("last seen {date} UTC")
            };
            card.push_str(&format!(
                "<li><strong>{}</strong> <code>{}</code><br>{}<br>{} · {} · revision {} {} {}</li>",
                escape(title.as_deref().unwrap_or(id)),
                escape(id),
                escape(&path),
                escape(&status),
                timing,
                rev,
                escape(hash.as_deref().unwrap_or("")),
                escape(reason.as_deref().unwrap_or("")),
            ));
        }
        if action == Action::Info {
            let mut labels =
                db.prepare("SELECT name,value FROM labels WHERE record_id=? ORDER BY name")?;
            for pair in labels.query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })? {
                let (key, value) = pair?;
                card.push_str(&format!("<li>{}: {}</li>", escape(&key), escape(&value)));
            }
            let mut aliases =
                db.prepare("SELECT DISTINCT alias FROM aliases WHERE record_id=? ORDER BY alias")?;
            for alias in aliases.query_map([id], |r| r.get::<_, String>(0))? {
                card.push_str(&format!("<li>Alias: {}</li>", escape(&alias?)));
            }
            let mut history = db.prepare(
                "SELECT path,revision_seq,sha256 FROM locations WHERE record_id=? AND is_current=0 AND revision_seq>0 ORDER BY id DESC",
            )?;
            for result in history.query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })? {
                let (path, revision, hash) = result?;
                card.push_str(&format!(
                    "<li>Historical revision {} · {} · {}</li>",
                    revision,
                    escape(&path),
                    escape(hash.as_deref().unwrap_or(""))
                ));
            }
            let mut links = db.prepare(
                "SELECT type,source_revision,source_sha256,
                  EXISTS(SELECT 1 FROM locations WHERE record_id=?1 AND is_current=1 AND status='active'
                         AND (links.source_revision IS NULL OR revision_seq=links.source_revision)
                         AND (links.source_sha256 IS NULL OR sha256=links.source_sha256))
                 FROM links WHERE from_id=?1 AND (source_revision IS NOT NULL OR source_sha256 IS NOT NULL)",
            )?;
            for result in
                links.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(3)?)))?
            {
                let (kind, current) = result?;
                card.push_str(&format!(
                    "<li>{} · {}</li>",
                    escape(&kind),
                    if current {
                        "matching source revision"
                    } else {
                        "historical / source-stale"
                    }
                ));
            }
        }
    }
    if ids.len() == 1 && !conflict && action == Action::Map {
        let mut query = db.prepare("SELECT DISTINCT value FROM labels WHERE record_id=? AND name='map_path' AND review_status IN ('reviewed','ruled')")?;
        let maps = query
            .query_map(params![ids.first()], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if let [map] = maps.as_slice() {
            let path = PathBuf::from(map);
            if path.is_absolute() && path.is_file() {
                return Ok(Resolution::Path(path, false));
            }
        }
        bail!("No uniquely registered existing map for {name}");
    }
    if ids.len() == 1
        && !conflict
        && matches!(action, Action::Open | Action::Folder)
        && let [path] = active.as_slice()
    {
        return Ok(Resolution::Path(path.clone(), action == Action::Folder));
    }
    let heading = if conflict {
        "Identity conflict"
    } else if ids.len() > 1 || active.len() > 1 {
        "Multiple matches"
    } else if action == Action::Info {
        "Record information"
    } else {
        "Missing or unavailable location"
    };
    if heading == "Multiple matches" {
        for id in &ids {
            card.push_str(&format!(
                "<li><a href=\"{}?info\">Record {}</a></li>",
                link(id),
                escape(id)
            ));
        }
        for path in &active {
            if let Ok(uri) = file_uri(path) {
                card.push_str(&format!(
                    "<li><a href=\"{}\">Open {}</a></li>",
                    escape(&uri),
                    escape(&path.display().to_string())
                ));
            }
        }
    }
    Ok(Resolution::Card(format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>{heading}</title><h1>{heading}</h1><p>{}</p><ul>{card}</ul></html>",
        escape(&name)
    )))
}

fn file_uri(path: &Path) -> Result<String> {
    let path = path.to_str().context("Non-Unicode path")?;
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    let encoded = path
        .split('/')
        .map(|part| {
            if cfg!(windows) && part.len() == 2 && part.ends_with(':') {
                part.to_owned()
            } else {
                utf8_percent_encode(part, NON_ALPHANUMERIC).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    if let Some(unc) = encoded.strip_prefix("//") {
        Ok(format!("file://{unc}"))
    } else {
        Ok(format!(
            "file://{}{}",
            if encoded.starts_with('/') { "" } else { "/" },
            encoded
        ))
    }
}

pub fn dispatch(
    resolution: Resolution,
    output: &Path,
    opener: impl FnOnce(&Path, bool) -> Result<()>,
) -> Result<()> {
    match resolution {
        Resolution::Path(path, select) => opener(&path, select),
        Resolution::Card(html) => {
            fs::create_dir_all(output)?;
            let mut file = tempfile::Builder::new()
                .prefix("tp-card-")
                .suffix(".html")
                .tempfile_in(output)?;
            file.write_all(html.as_bytes())?;
            let (_, path) = file.keep()?;
            opener(&path, false)
        }
    }
}

pub fn path_id(db: &Connection, path: &Path) -> Result<String> {
    let path = crate::platform::scan_path(path)?;
    let mut query = db.prepare("SELECT path,record_id FROM locations WHERE is_current=1 AND status='active' AND record_id IS NOT NULL")?;
    for row in query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (stored, id) = row?;
        let equal = path == Path::new(&stored)
            || cfg!(windows) && path.to_string_lossy().to_lowercase() == stored.to_lowercase();
        if equal {
            return Ok(id);
        }
    }
    bail!("No active record for {}", path.display())
}
