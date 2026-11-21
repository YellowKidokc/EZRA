use crate::{
    anchor::{self, Anchor},
    index::{self, Location},
    platform,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use uuid::Uuid;
use walkdir::WalkDir;

static SERIES: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"\b[A-Z][A-Z0-9]+\.\d{2}\.[A-Z][A-Z0-9_-]*\b").unwrap()
});

#[derive(Default, Serialize)]
pub struct Receipt {
    requested: Vec<String>,
    completed: Vec<String>,
    unavailable: Vec<String>,
    excluded: Vec<String>,
    errors: Vec<String>,
    files: usize,
    folders: usize,
    new: usize,
    unchanged: usize,
    edited: usize,
    moved: usize,
    missing: usize,
    unknown: usize,
    duplicate_id_conflicts: usize,
    ambiguous: usize,
    review: Vec<Hold>,
    elapsed_ms: u128,
}

#[derive(Serialize)]
struct Hold {
    path: String,
    reason: String,
}

struct Item {
    path: String,
    kind: &'static str,
    hash: Option<String>,
    size: Option<i64>,
    mtime: Option<String>,
    anchor: Anchor,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn observe(path: &Path, key: String, prior: Option<&Location>, full_hash: bool) -> Result<Item> {
    let directory = path.is_dir();
    let mut item = Item {
        path: key,
        kind: if directory { "folder" } else { "file" },
        hash: None,
        size: None,
        mtime: None,
        anchor: Anchor::Absent,
    };
    if directory {
        return Ok(item);
    }
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        bail!("unsupported file type");
    }
    item.size = Some(i64::try_from(metadata.len())?);
    item.mtime = metadata
        .modified()
        .ok()
        .map(|time| match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos().to_string(),
            Err(e) => format!("-{}", e.duration().as_nanos()),
        });
    if !anchor::supported(path)
        && !full_hash
        && let Some(old) = prior
        && old.status == "active"
        && old.hash.is_some()
        && old.size == item.size
        && item.mtime.is_some()
        && old.mtime == item.mtime
    {
        item.hash.clone_from(&old.hash);
    } else if anchor::supported(path) {
        let bytes = fs::read(path)?;
        item.hash = Some(digest(&bytes));
        item.anchor = anchor::read(path, &bytes);
    } else {
        let mut hash = Sha256::new();
        io::copy(&mut fs::File::open(path)?, &mut hash)?;
        item.hash = Some(format!("{:x}", hash.finalize()));
    }
    let after = fs::metadata(path)?;
    if after.len() != metadata.len() || after.modified()? != metadata.modified()? {
        bail!("file changed during observation");
    }
    Ok(item)
}

fn append(
    db: &Connection,
    item: &Item,
    record: Option<&str>,
    prior: Option<&Location>,
    stamp: i64,
    revision: i64,
    status: &str,
    reason: Option<&str>,
) -> Result<()> {
    if let Some(old) = prior {
        db.execute("UPDATE locations SET is_current=0 WHERE id=?", [old.row])?;
    }
    let claimed = match &item.anchor {
        Anchor::Valid(value) => Some(value.as_str()),
        _ => None,
    };
    db.execute(
        "INSERT INTO locations(record_id,claimed_id,path,sha256,size,mtime,first_seen,last_seen,observed_at,revision_seq,status,kind,reason)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
        params![record, claimed, item.path, item.hash, item.size, item.mtime, prior.map_or(stamp, |p| p.first),
                stamp, stamp, revision, status, item.kind, reason],
    )?;
    Ok(())
}

fn under_any(path: &Path, roots: &[PathBuf]) -> bool {
    #[cfg(windows)]
    {
        let key = path.to_string_lossy().to_lowercase();
        return roots
            .iter()
            .any(|root| Path::new(&key).starts_with(root.to_string_lossy().to_lowercase()));
    }
    #[cfg(not(windows))]
    roots.iter().any(|root| path.starts_with(root))
}

#[cfg(windows)]
fn path_key(path: &str) -> String {
    path.replace('/', "\\").to_lowercase()
}
#[cfg(not(windows))]
fn path_key(path: &str) -> String {
    path.to_owned()
}

fn attempt_insert(item: &Item, id: &str) -> Result<Option<Item>> {
    let path = Path::new(&item.path);
    let metadata = fs::metadata(path)?;
    let bytes = fs::read(path)?;
    if item.hash.as_deref() != Some(&digest(&bytes)) {
        bail!("file changed before ID insertion");
    }
    let Some(output) = anchor::insert(path, &bytes, id) else {
        return Ok(None);
    };
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().context("no parent for source file")?)?;
    temp.as_file().set_permissions(metadata.permissions())?;
    temp.write_all(&output)?;
    temp.as_file().sync_all()?;
    if fs::read(path)? != bytes {
        bail!("file changed before ID insertion");
    }
    temp.persist(path)?;
    let after = observe(path, item.path.clone(), None, true)?;
    if after.hash.as_deref() != Some(&digest(&output)) {
        bail!("file changed after ID insertion");
    }
    Ok(Some(after))
}

fn next_revision(db: &Connection, id: &str, hash: Option<&str>) -> Result<i64> {
    Ok(db.query_row(
        "SELECT COALESCE(MAX(CASE WHEN sha256 IS ? THEN revision_seq END), COALESCE(MAX(revision_seq),0)+1)
         FROM locations WHERE record_id=? AND revision_seq>0",
        params![hash, id], |r| r.get(0),
    )?)
}

fn save_review(
    db: &Connection,
    receipt: &mut Receipt,
    item: &Item,
    old: Option<&Location>,
    stamp: i64,
    reason: &str,
) -> Result<()> {
    let prior_record = old.and_then(|p| p.record.as_deref());
    if let Some(old) = old
        && old.status == "conflict"
        && old.hash == item.hash
    {
        let same: bool = db.query_row(
            "SELECT reason IS ? AND claimed_id IS ? FROM locations WHERE id=?",
            params![
                reason,
                match &item.anchor {
                    Anchor::Valid(id) => Some(id),
                    _ => None,
                },
                old.row
            ],
            |row| row.get(0),
        )?;
        if same {
            db.execute(
                "UPDATE locations SET last_seen=? WHERE id=?",
                params![stamp, old.row],
            )?;
            receipt.review.push(Hold {
                path: item.path.clone(),
                reason: reason.into(),
            });
            return Ok(());
        }
    }
    append(
        db,
        item,
        prior_record,
        old,
        stamp,
        0,
        "conflict",
        Some(reason),
    )?;
    receipt.review.push(Hold {
        path: item.path.clone(),
        reason: reason.into(),
    });
    Ok(())
}

pub fn run(
    db: &mut Connection,
    roots: &[PathBuf],
    skips: &[String],
    full_hash: bool,
    write_ids: bool,
    database: &Path,
    output: &Path,
) -> Result<()> {
    let timer = std::time::Instant::now();
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let stamp = index::now();
    let old_list = index::locations(&tx)?;
    let old: HashMap<_, _> = old_list
        .iter()
        .map(|location| (path_key(&location.path), location))
        .collect();
    let mut receipt = Receipt::default();
    let mut covered = Vec::new();
    let mut blocked = Vec::new();
    let mut seen = HashSet::new();
    let mut items = BTreeMap::new();
    let mut path_aliases: HashMap<String, Vec<String>> = HashMap::new();
    let mut skips: HashSet<&str> = skips.iter().map(String::as_str).collect();
    skips.extend([".git", "node_modules", ".lake", "__pycache__", ".obsidian"]);
    for requested in roots {
        let input = std::path::absolute(requested)?;
        receipt.requested.push(input.display().to_string());
        let root = match platform::scan_path(&input) {
            Ok(path) if path.is_dir() || path.is_file() => path,
            _ => {
                blocked.push(input.clone());
                receipt.unavailable.push(input.display().to_string());
                continue;
            }
        };
        if root.starts_with(output) {
            bail!(
                "Scan root is within the tool output directory: {}",
                root.display()
            );
        }
        let mut complete = true;
        let walker = WalkDir::new(&root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| {
                let path = entry.path();
                let internal = path == database
                    || path.starts_with(output)
                    || ["-wal", "-shm", "-journal"].iter().any(|suffix| {
                        path.as_os_str().to_string_lossy()
                            == format!("{}{suffix}", database.display())
                    });
                let excluded = internal
                    || entry.file_type().is_symlink()
                    || entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| skips.contains(name));
                if excluded {
                    blocked.push(path.to_owned());
                    receipt.excluded.push(path.display().to_string());
                }
                !excluded
            });
        let mut failures = Vec::new();
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    complete = false;
                    failures.push(error.path().unwrap_or(&root).to_owned());
                    receipt.errors.push(error.to_string());
                    continue;
                }
            };
            let path = entry.path();
            let Some(text) = path.to_str() else {
                complete = false;
                failures.push(path.to_owned());
                receipt.unknown += 1;
                receipt
                    .errors
                    .push(format!("Non-Unicode path: {}", path.display()));
                continue;
            };
            let key = path_key(text);
            if input != root
                && let Ok(relative) = path.strip_prefix(&root)
                && let Some(alias) = input.join(relative).to_str()
            {
                path_aliases
                    .entry(key.clone())
                    .or_default()
                    .push(alias.to_owned());
            }
            if !seen.insert(key.clone()) {
                continue;
            }
            match observe(path, text.to_owned(), old.get(&key).copied(), full_hash) {
                Ok(item) => {
                    items.insert(key, item);
                }
                Err(error) => {
                    complete = false;
                    failures.push(path.to_owned());
                    receipt
                        .errors
                        .push(format!("{}: {error:#}", path.display()));
                    receipt.unknown += 1;
                }
            }
        }
        blocked.extend(failures);
        if complete {
            receipt.completed.push(root.display().to_string());
        } else {
            receipt.unavailable.push(root.display().to_string());
        }
        covered.push(root);
    }

    let mut missing = Vec::new();
    let mut newly_missing = HashSet::new();
    let mut absent = HashSet::new();
    for location in &old_list {
        let path = Path::new(&location.path);
        let key = path_key(&location.path);
        if items.contains_key(&key) {
            continue;
        }
        if under_any(path, &blocked) || seen.contains(&key) {
            if location.status == "active" || location.status == "unknown" {
                tx.execute(
                    "UPDATE locations SET status='unknown' WHERE id=?",
                    [location.row],
                )?;
                receipt.unknown += 1;
            }
        } else if under_any(path, &covered) {
            absent.insert(location.row);
            if matches!(location.status.as_str(), "active" | "unknown" | "conflict") {
                tx.execute(
                    "UPDATE locations SET status='missing', missing_since=? WHERE id=?",
                    params![stamp, location.row],
                )?;
                receipt.missing += 1;
                if location.status != "conflict" {
                    newly_missing.insert(location.row);
                }
            }
            if location.record.is_some() && location.kind == "file" && location.revision > 0 {
                missing.push(location);
            }
        }
    }

    let mut anchors: HashMap<&str, Vec<&Item>> = HashMap::new();
    let mut observed_identity: HashMap<&str, Vec<&Item>> = HashMap::new();
    let mut incoming_hashes: HashMap<&str, usize> = HashMap::new();
    for (key, item) in &items {
        if let Anchor::Valid(id) = &item.anchor {
            anchors.entry(id).or_default().push(item);
        }
        let previous = old.get(key).and_then(|location| location.record.as_deref());
        let identity = match &item.anchor {
            Anchor::Valid(id) => Some(id.as_str()),
            _ => previous,
        };
        if let Some(id) = identity {
            observed_identity.entry(id).or_default().push(item);
        }
        if !old.get(key).is_some_and(|p| p.record.is_some())
            && matches!(item.anchor, Anchor::Absent)
            && let Some(hash) = item.hash.as_deref()
        {
            *incoming_hashes.entry(hash).or_default() += 1;
        }
    }
    let mut missing_hashes: HashMap<&str, Vec<&Location>> = HashMap::new();
    let mut by_record: HashMap<&str, Vec<&Location>> = HashMap::new();
    let mut by_claim: HashMap<&str, Vec<&Location>> = HashMap::new();
    for location in &old_list {
        if let Some(id) = location.claimed.as_deref() {
            by_claim.entry(id).or_default().push(location);
        }
        if let Some(id) = location.record.as_deref() {
            by_record.entry(id).or_default().push(location);
        }
    }
    for &candidate in &missing {
        if let Some(hash) = candidate.hash.as_deref() {
            missing_hashes.entry(hash).or_default().push(candidate);
        }
    }

    let mut plans = Vec::new();
    for (key, item) in &items {
        if item.kind == "folder" {
            receipt.folders += 1;
        } else {
            receipt.files += 1;
        }
        let prior = old.get(key).copied();
        let prior_id = prior.and_then(|p| p.record.as_deref());
        let mut source = None;
        let mut record = prior_id.map(str::to_owned);
        let mut reason = None;
        match &item.anchor {
            Anchor::Conflict => {
                reason = Some("Malformed, conflicting or unsupported identity metadata")
            }
            Anchor::Valid(id) => {
                let different_observation = observed_identity[id.as_str()]
                    .iter()
                    .any(|copy| copy.hash != item.hash);
                let different_unscanned_copy = by_record
                    .get(id.as_str())
                    .into_iter()
                    .flatten()
                    .chain(by_claim.get(id.as_str()).into_iter().flatten())
                    .any(|location| {
                        !items.contains_key(&path_key(&location.path))
                            && !absent.contains(&location.row)
                            && location.status != "missing"
                            && location.hash != item.hash
                    });
                if prior_id.is_some_and(|previous| previous != id)
                    || prior.is_some_and(|old| old.record.is_some() && old.kind != item.kind)
                {
                    reason = Some("Trusted header contradicts the existing identity at this path");
                } else if different_observation || different_unscanned_copy {
                    reason = Some("Duplicate anchor at different paths with different content");
                    receipt.duplicate_id_conflicts += 1;
                } else {
                    record = Some(id.clone());
                    let mut candidates =
                        missing.iter().filter(|old| old.record.as_ref() == Some(id));
                    if prior_id.is_none() && anchors[id.as_str()].len() == 1 {
                        source = candidates
                            .next()
                            .copied()
                            .filter(|_| candidates.next().is_none());
                    }
                }
            }
            Anchor::Absent => {
                if prior_id.is_some_and(|id| {
                    observed_identity[id]
                        .iter()
                        .any(|copy| copy.hash != item.hash)
                }) {
                    reason = Some("Duplicate anchor at different paths with different content");
                    receipt.duplicate_id_conflicts += 1;
                } else if prior.is_some_and(|p| p.record.is_some() && p.kind != item.kind) {
                    reason = Some("A different item kind replaced this path");
                } else if prior_id.is_none()
                    && let Some(hash) = item.hash.as_deref()
                {
                    if let Some(candidates) = missing_hashes.get(hash) {
                        if candidates.len() == 1
                            && incoming_hashes[hash] == 1
                            && candidates[0].record.as_deref().is_some_and(|id| {
                                !anchors.contains_key(id)
                                    && by_record[id].iter().all(|other| {
                                        other.row == candidates[0].row || other.status == "missing"
                                    })
                            })
                        {
                            source = Some(candidates[0]);
                            record.clone_from(&candidates[0].record);
                        } else {
                            reason = Some(
                                "Full hash matches an ambiguous or independently claimed location",
                            );
                        }
                    }
                }
            }
        }
        plans.push((item, prior, record, source, reason));
    }
    let claimed_moves: HashSet<_> = plans
        .iter()
        .filter_map(|(_, _, _, source, reason)| reason.is_none().then_some(source.as_ref()?.row))
        .collect();
    let unmatched_missing = missing
        .iter()
        .any(|m| newly_missing.contains(&m.row) && !claimed_moves.contains(&m.row));

    for (item, prior, mut record, source, mut reason) in plans {
        if reason.is_none()
            && record.is_none()
            && item.kind == "file"
            && (unmatched_missing || prior.is_some_and(|p| p.status == "conflict"))
        {
            reason = Some("New path cannot be distinguished from an unanchored edit plus move");
        }
        if let Some(reason) = reason {
            if !reason.starts_with("Duplicate anchor") {
                receipt.ambiguous += 1;
            }
            save_review(&tx, &mut receipt, item, prior, stamp, reason)?;
            continue;
        }
        let id = record.take().unwrap_or_else(|| Uuid::now_v7().to_string());
        let title = Path::new(&item.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&item.path);
        tx.execute(
            "INSERT OR IGNORE INTO records(id,kind,title,created_at) VALUES (?,?,?,?)",
            params![id, item.kind, title, stamp],
        )?;
        if !title.trim().is_empty() && !title.chars().any(char::is_control) {
            index::alias(&tx, &id, title, "filename")?;
        }
        for alias in path_aliases
            .get(&path_key(&item.path))
            .into_iter()
            .flatten()
        {
            index::alias(&tx, &id, alias, "path-spelling")?;
        }
        for alias in SERIES.find_iter(title) {
            index::alias(&tx, &id, alias.as_str(), "series-id")?;
        }
        let revision = next_revision(&tx, &id, item.hash.as_deref())?;
        if let Some(source) = source {
            tx.execute(
                "UPDATE locations SET is_current=0, status='moved' WHERE id=?",
                [source.row],
            )?;
            receipt.moved += 1;
            if source.status != "missing" {
                receipt.missing -= 1;
            }
        } else if let Some(previous) = prior.filter(|p| p.record.as_deref() == Some(id.as_str())) {
            if previous.hash == item.hash && previous.status == "active" {
                tx.execute(
                    "UPDATE locations SET last_seen=?, size=?, mtime=? WHERE id=?",
                    params![stamp, item.size, item.mtime, previous.row],
                )?;
                receipt.unchanged += 1;
                if !write_ids
                    || !anchor::supported(Path::new(&item.path))
                    || !matches!(item.anchor, Anchor::Absent)
                {
                    continue;
                }
            } else if previous.hash != item.hash {
                receipt.edited += 1;
            }
        } else {
            receipt.new += 1;
        }
        let unchanged = prior.is_some_and(|p| {
            p.status == "active" && p.record.as_deref() == Some(&id) && p.hash == item.hash
        });
        if !unchanged {
            append(&tx, item, Some(&id), prior, stamp, revision, "active", None)?;
        }
        if write_ids
            && anchor::supported(Path::new(&item.path))
            && matches!(item.anchor, Anchor::Absent)
        {
            match attempt_insert(item, &id) {
                Ok(Some(after)) => {
                    let updated = index::locations_at(&tx, &item.path)?
                        .context("pre-write observation missing")?;
                    let revision = next_revision(&tx, &id, after.hash.as_deref())?;
                    append(
                        &tx,
                        &after,
                        Some(&id),
                        Some(&updated),
                        stamp,
                        revision,
                        "active",
                        Some("explicit ID insertion"),
                    )?;
                }
                Ok(None) => receipt.review.push(Hold {
                    path: item.path.clone(),
                    reason: "Unsupported layout for header insertion".into(),
                }),
                Err(error) => receipt
                    .errors
                    .push(format!("{}: ID insertion: {error:#}", item.path)),
            }
        }
    }
    tx.commit()?;
    receipt.elapsed_ms = timer.elapsed().as_millis();
    fs::create_dir_all(output)?;
    let run_directory = tempfile::Builder::new()
        .prefix("scan-")
        .tempdir_in(output)?
        .keep();
    let csv_path = run_directory.join("needs_review.csv");
    let mut csv_file = tempfile::NamedTempFile::new_in(&run_directory)?;
    {
        let mut csv = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(csv_file.as_file_mut());
        csv.write_record(["path", "reason"])?;
        for hold in &receipt.review {
            csv.serialize(hold)?;
        }
        csv.flush()?;
    }
    csv_file.persist_noclobber(csv_path)?;
    let mut json = tempfile::NamedTempFile::new_in(&run_directory)?;
    serde_json::to_writer_pretty(json.as_file_mut(), &receipt)?;
    json.persist_noclobber(run_directory.join("scan-receipt.json"))?;
    eprintln!("Scan artifacts: {}", run_directory.display());
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
