use super::{index, linker, platform, resolver, scan};
use anyhow::Result;
use resolver::{Action, Resolution};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use uuid::Uuid;

struct Fixture {
    temp: TempDir,
    root: PathBuf,
    database: PathBuf,
    output: PathBuf,
    db: Connection,
}

impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let base = platform::absolute(temp.path())?;
        let root = base.join("corpus");
        fs::create_dir(&root)?;
        let database = base.join("test.sqlite");
        let output = base.join("output");
        let db = index::open(&database)?;
        Ok(Self {
            temp,
            root,
            database,
            output,
            db,
        })
    }

    fn write(&self, name: &str, content: impl AsRef<[u8]>) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, content)?;
        Ok(path)
    }

    fn scan(&mut self) -> Result<()> {
        self.scan_roots(&[self.root.clone()], &[], false)
    }

    fn scan_roots(&mut self, roots: &[PathBuf], skips: &[String], write_ids: bool) -> Result<()> {
        scan::run(
            &mut self.db,
            roots,
            skips,
            true,
            write_ids,
            &self.database,
            &self.output,
        )
    }

    fn id(&self, path: &Path) -> Result<String> {
        resolver::path_id(&self.db, path)
    }

    fn location(&self, path: &Path) -> index::Location {
        index::locations_at(&self.db, path.to_str().unwrap())
            .unwrap()
            .unwrap()
    }

    fn find_card(&self, uri: &str) -> Result<String> {
        match resolver::resolve(&self.db, uri)? {
            Resolution::Card(s) => Ok(s),
            Resolution::Path(path, _) => panic!("expected card, got {}", path.display()),
        }
    }

    fn assert_open(&self, uri: &str, expected: &Path) -> Result<()> {
        let resolution = resolver::resolve(&self.db, uri)?;
        let mut opened = false;
        resolver::dispatch(resolution, &self.output, |path, select| {
            assert_eq!(path, expected);
            assert!(!select);
            opened = true;
            Ok(())
        })?;
        assert!(opened);
        Ok(())
    }
}

#[test]
fn check_01_mixed_inventory() -> Result<()> {
    let mut f = Fixture::new()?;
    for i in 0..20 {
        let ext = ["md", "html", "pdf", "png", "zip"][i % 5];
        f.write(
            &format!("group{}/test{i}.{ext}", i % 2),
            format!("Fixture {i}"),
        )?;
    }
    f.scan()?;
    let locations = index::locations(&f.db)?;
    assert_eq!(locations.iter().filter(|l| l.kind == "file").count(), 20);
    assert_eq!(locations.iter().filter(|l| l.kind == "folder").count(), 3);
    assert!(
        locations
            .iter()
            .all(|l| l.status == "active" && l.record.is_some())
    );
    assert!(
        locations
            .iter()
            .filter(|l| l.kind == "folder")
            .all(|l| l.hash.is_none())
    );
    Ok(())
}

#[test]
fn checks_02_03_04_resolve_move_then_missing() -> Result<()> {
    let mut f = Fixture::new()?;
    let original = f.write("first.txt", "identity")?;
    f.scan()?;
    let id = f.id(&original)?;
    let uri = resolver::link(&id);
    index::alias(&f.db, &id, "Résumé Draft", "manual")?;
    f.assert_open(&uri, &original)?;
    f.assert_open(&resolver::link("RÉSUMÉ draft"), &original)?;
    let renamed = f.root.join("renamed.txt");
    fs::rename(&original, &renamed)?;
    f.scan()?;
    assert_eq!(f.id(&renamed)?, id);
    f.assert_open(&uri, &renamed)?;
    assert_eq!(
        f.db.query_row(
            "SELECT COUNT(DISTINCT revision_seq) FROM locations WHERE record_id=?",
            [&id],
            |row| row.get::<_, u32>(0)
        )?,
        1
    );
    fs::remove_file(&renamed)?;
    f.scan()?;
    let card = f.find_card(&uri)?;
    assert!(card.contains("missing") && card.contains("renamed.txt"));
    assert_eq!(f.location(&renamed).status, "missing");
    Ok(())
}

#[test]
fn check_05_duplicate_hash_hold() -> Result<()> {
    let mut f = Fixture::new()?;
    let a = f.write("a.zip", "same bytes")?;
    let b = f.write("b.zip", "same bytes")?;
    f.scan()?;
    let a2 = f.root.join("a2.zip");
    let b2 = f.root.join("b2.zip");
    fs::rename(a, &a2)?;
    fs::rename(b, &b2)?;
    f.scan()?;
    for path in [&a2, &b2] {
        assert_eq!(f.location(path).status, "conflict");
        assert!(f.location(path).record.is_none());
    }
    let csv = fs::read_dir(&f.output)?
        .filter_map(Result::ok)
        .map(|entry| fs::read_to_string(entry.path().join("needs_review.csv")))
        .collect::<std::io::Result<Vec<_>>>()?;
    assert!(
        csv.iter()
            .any(|text| text.contains("a2.zip") && text.contains("b2.zip"))
    );
    Ok(())
}

#[test]
fn check_06_linkify_copy_and_collision() -> Result<()> {
    let mut f = Fixture::new()?;
    let paper = f.write("paper.pdf", "target")?;
    let another = f.write("another.pdf", "another target")?;
    f.scan()?;
    let id = f.id(&paper)?;
    index::alias(&f.db, &id, "Core Paper", "manual")?;
    index::alias(&f.db, &id, "Unclear", "manual")?;
    index::alias(&f.db, &f.id(&another)?, "Unclear", "manual")?;
    let originals = [
        (
            "view.md",
            format!(
                "Core Paper or {}. Unclear. `Core Paper` [Core Paper](https://example.org) https://example.org/paper.pdf",
                paper.display()
            ),
        ),
        (
            "view.html",
            format!(
                "<p>Core Paper or {}. Unclear.</p><a href=\"#\">Core Paper</a>",
                paper.display()
            ),
        ),
    ];
    for (name, content) in originals {
        let input = f.write(name, &content)?;
        let out = f.temp.path().join("linked");
        let linked = linker::run(&f.db, &input, &out)?;
        let result = fs::read_to_string(linked)?;
        assert_eq!(result.matches(&resolver::link(&id)).count(), 2);
        assert!(result.contains("Unclear"));
        if name.ends_with(".md") {
            assert!(result.contains("https://example.org/paper.pdf"));
        }
        assert_eq!(fs::read(&input)?, content.as_bytes());
        assert!(linker::run(&f.db, &input, &out).is_err());
        assert!(linker::run(&f.db, &input, &f.root).is_err());
    }
    Ok(())
}

#[test]
fn check_07_local_database_gate() -> Result<()> {
    let f = Fixture::new()?;
    for path in [
        r"\\nas\share\tp.sqlite",
        "//nas/share/tp.sqlite",
        r"\\?\UNC\nas\share\tp.sqlite",
    ] {
        assert!(platform::local_path(Path::new(path)).is_err());
    }
    let local = f.temp.path().join("deep/local/db.sqlite");
    index::open(&local)?;
    assert!(local.is_file());
    let existing = f.temp.path().join("unrelated.sqlite");
    let foreign_db = Connection::open(&existing)?;
    foreign_db.execute("CREATE TABLE external (value TEXT)", [])?;
    drop(foreign_db);
    let before = fs::read(&existing)?;
    assert!(index::open(&existing).is_err());
    assert_eq!(fs::read(&existing)?, before);
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("EZRA_TEST_MAPPED_ROOT") {
        assert!(platform::local_path(&PathBuf::from(root).join("ezra-test.sqlite")).is_err());
    }
    #[cfg(target_os = "linux")]
    assert!(platform::local_path(Path::new("/proc/ezra-test.sqlite")).is_err());
    Ok(())
}

#[test]
fn checks_08_21_handler_formation() {
    assert_eq!(
        platform::handler(Path::new(r"C:\Portable Apps\EZRA\tp.exe"), "open"),
        r#""C:\Portable Apps\EZRA\tp.exe" open -- "%1""#,
    );
    assert_eq!(
        platform::handler(Path::new(r"C:\Portable Apps\EZRA\tp.exe"), "link"),
        r#""C:\Portable Apps\EZRA\tp.exe" link -- "%1""#,
    );
}

#[test]
fn check_10_edits_and_receipt_scope() -> Result<()> {
    let mut f = Fixture::new()?;
    let paths: Vec<_> = ["paper.pdf", "paper.docx", "note.txt"]
        .into_iter()
        .map(|name| f.write(name, "initial version"))
        .collect::<Result<_>>()?;
    f.scan()?;
    let notes = paths
        .iter()
        .map(|path| Ok((f.id(path)?, f.location(path).hash.unwrap())))
        .collect::<Result<Vec<_>>>()?;
    for (path, (id, hash)) in paths.iter().zip(&notes) {
        f.db.execute("INSERT INTO links(from_id,to_id,type,status,source_revision,source_sha256) VALUES(?1,?1,'proof receipt','reviewed',1,?2)", params![id, hash])?;
        fs::write(path, "edited version")?;
    }
    f.scan()?;
    for (path, (id, hash)) in paths.iter().zip(&notes) {
        assert_eq!(&f.id(path)?, id);
        let mut query = f
            .db
            .prepare("SELECT revision_seq,sha256 FROM locations WHERE record_id=? ORDER BY id")?;
        let history = query
            .query_map([id], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(history.len(), 2);
        assert_eq!(&history[0], &(1, hash.clone()));
        assert_eq!(history[1].0, 2);
        assert!(
            f.find_card(&format!("tp://{id}?info"))?
                .contains("historical / source-stale")
        );
    }
    Ok(())
}

#[test]
fn check_11_coverage_gates_missing() -> Result<()> {
    let mut f = Fixture::new()?;
    let path = f.write("first.txt", "primary")?;
    let excluded = f.write("drafts/excluded.txt", "draft")?;
    let second = platform::absolute(f.temp.path())?.join("other");
    fs::create_dir(&second)?;
    let other = second.join("other.txt");
    fs::write(&other, "other")?;
    let root = f.root.clone();
    f.scan_roots(&[root.clone(), second.clone()], &[], false)?;
    fs::remove_file(&path)?;
    fs::remove_file(&excluded)?;
    fs::remove_file(&other)?;
    f.scan_roots(&[root], &["drafts".into()], false)?;
    assert_eq!(f.location(&path).status, "missing");
    assert_eq!(f.location(&excluded).status, "unknown");
    assert_eq!(f.location(&other).status, "active");
    fs::rename(&second, second.with_extension("offline"))?;
    f.scan_roots(&[second], &[], false)?;
    assert_eq!(f.location(&other).status, "unknown");
    Ok(())
}

#[test]
fn check_12_anchor_replacement_and_atomic_save() -> Result<()> {
    let mut f = Fixture::new()?;
    let changed = f.write("paper.txt", "old text")?;
    let anchored = f.write("anchored.md", "no anchor")?;
    f.scan()?;
    let id = f.id(&changed)?;
    let previous = f.id(&anchored)?;
    let replacement = tempfile::NamedTempFile::new_in(&f.root)?;
    fs::write(replacement.path(), "normal atomic save")?;
    replacement.persist(&changed)?;
    let claimed = Uuid::now_v7();
    fs::write(
        &anchored,
        format!("---\nrecord_id: {claimed}\n---\nContradiction"),
    )?;
    f.scan()?;
    assert_eq!(f.id(&changed)?, id);
    let hold = f.location(&anchored);
    assert_eq!(hold.status, "conflict");
    assert_eq!(hold.record.as_deref(), Some(previous.as_str()));
    assert!(
        f.find_card(&format!("tp://{claimed}"))?
            .contains("Identity conflict")
    );
    Ok(())
}

#[test]
fn check_13_duplicate_anchors_and_aliases() -> Result<()> {
    let mut f = Fixture::new()?;
    let anchor = Uuid::now_v7();
    let a = f.write("first.md", format!("---\nuuid: {anchor}\n---\nFirst"))?;
    let b = f.write("second.md", format!("---\nuuid: {anchor}\n---\nSecond"))?;
    let c = f.write("c.txt", "one")?;
    let d = f.write("d.txt", "two")?;
    f.scan()?;
    for path in [&a, &b] {
        assert_eq!(f.location(path).status, "conflict");
    }
    let card = f.find_card(&format!("tp://{anchor}"))?;
    assert!(card.contains("first.md") && card.contains("second.md"));
    for path in [&c, &d] {
        index::alias(&f.db, &f.id(path)?, "Same Alias", "manual")?;
    }
    assert!(
        f.find_card(&resolver::link("Same Alias"))?
            .contains("Multiple matches")
    );
    let identical = Uuid::now_v7();
    for name in ["copy1.md", "copy2.md"] {
        f.write(name, format!("---\nrecord_id: {identical}\n---\nSame text"))?;
    }
    f.scan()?;
    assert!(
        f.find_card(&format!("tp://{identical}"))?
            .contains("Multiple matches")
    );
    Ok(())
}

#[test]
fn check_14_opt_in_metadata_preserves_bytes() -> Result<()> {
    let mut f = Fixture::new()?;
    let md = "\u{feff}---\r\ntitle: Café\r\n---\r\nContent\r\n";
    let html =
        "<!doctype html>\r\n<html><head><title>Café</title></head><body>Body</body></html>\r\n";
    let paths = [
        (f.write("note.md", md)?, md),
        (f.write("page.html", html)?, html),
    ];
    let unsupported = f.write("invalid.html", "<body>no head</body>")?;
    f.scan()?;
    for (path, bytes) in &paths {
        assert_eq!(&fs::read(path)?, bytes.as_bytes());
    }
    f.scan_roots(&[f.root.clone()], &[], true)?;
    for (path, original) in &paths {
        let id = f.id(path)?;
        let written = fs::read_to_string(path)?;
        let insert = if path.extension().unwrap() == "md" {
            format!("record_id: {id}\r\n")
        } else {
            format!("\r\n<meta name=\"record_id\" content=\"{id}\">")
        };
        assert_eq!(written.replacen(&insert, "", 1), *original);
        assert_eq!(
            f.location(path).hash,
            Some(format!("{:x}", Sha256::digest(written.as_bytes())))
        );
    }
    assert_eq!(fs::read_to_string(unsupported)?, "<body>no head</body>");
    f.scan()?;
    for (path, _) in &paths {
        let count = f.db.query_row(
            "SELECT COUNT(*) FROM locations WHERE record_id=?",
            [f.id(path)?],
            |row| row.get::<_, u32>(0),
        )?;
        assert_eq!(count, 2);
    }
    Ok(())
}

#[test]
fn check_15_full_hash_over_equal_samples() -> Result<()> {
    let mut f = Fixture::new()?;
    let mut left = vec![0u8; 300_000];
    let mut right = left.clone();
    left[193_001] = 1;
    right[193_001] = 2;
    let old = f.write("old.bin", left)?;
    f.scan()?;
    let id = f.id(&old)?;
    fs::remove_file(old)?;
    let candidate = f.write("candidate.bin", right)?;
    f.scan()?;
    assert_eq!(f.location(&candidate).status, "conflict");
    assert_ne!(f.location(&candidate).record.as_deref(), Some(id.as_str()));
    Ok(())
}

#[test]
fn check_16_uri_round_trips() -> Result<()> {
    for (input, action) in [
        ("Core Résumé", Action::Info),
        ("%20 literal", Action::Folder),
        ("A/B", Action::Map),
    ] {
        let flag = match action {
            Action::Info => "info",
            Action::Folder => "folder",
            Action::Map => "map",
            _ => unreachable!(),
        };
        assert_eq!(
            resolver::parse(&format!("{}/?{flag}=", resolver::link(input)))?,
            (input.into(), action)
        );
    }
    assert_eq!(
        resolver::parse("TP://AbCd/")?,
        ("AbCd".into(), Action::Open)
    );
    for input in [
        "tp://id/extra",
        "tp://id//",
        "tp://%%",
        "tp://id?unrecognized",
        "tp://id#anchor",
        "https://id",
    ] {
        assert!(resolver::parse(input).is_err(), "{input}");
    }
    Ok(())
}

#[test]
fn checks_17_18_folder_and_edited_file_relocation() -> Result<()> {
    let mut f = Fixture::new()?;
    let moving = f.write("folder/file.txt", "same")?;
    let edited = f.write("old.txt", "before")?;
    let anchor = Uuid::now_v7();
    let anchored = f.write(
        "anchor.md",
        format!("---\nrecord_id: {anchor}\n---\nBefore"),
    )?;
    f.scan()?;
    let folder = moving.parent().unwrap();
    let folder_id = f.id(folder)?;
    let other = f.root.join("different");
    fs::rename(folder, &other)?;
    fs::remove_file(edited)?;
    let changed = f.write("new.txt", "after")?;
    fs::remove_file(anchored)?;
    let relocated = f.write(
        "new-anchor.md",
        format!("---\nrecord_id: {anchor}\n---\nAfter"),
    )?;
    f.scan()?;
    assert_eq!(f.location(folder).status, "missing");
    assert_ne!(f.id(&other)?, folder_id);
    assert_eq!(f.location(&changed).status, "conflict");
    assert_eq!(f.id(&relocated)?, anchor.to_string());
    f.assert_open(&format!("tp://{anchor}"), &relocated)?;
    Ok(())
}

#[test]
fn check_20_concurrent_writer_uuidv7() -> Result<()> {
    let f = Fixture::new()?;
    let mut workers = Vec::new();
    for group in 0..6 {
        let root = f.temp.path().join(format!("parallel-{group}"));
        fs::create_dir(&root)?;
        for i in 0..30 {
            fs::write(root.join(format!("{i}.bin")), format!("{group}-{i}"))?;
        }
        let db = f.database.clone();
        let output = f.output.clone();
        workers.push(std::thread::spawn(move || -> Result<()> {
            let mut connection = index::open(&db)?;
            scan::run(&mut connection, &[root], &[], true, false, &db, &output)
        }));
    }
    for worker in workers {
        worker.join().unwrap()?;
    }
    let mut query = f
        .db
        .prepare("SELECT r.id FROM records r JOIN locations l ON l.record_id=r.id ORDER BY l.id")?;
    let ids = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(ids.len(), 186);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    for id in ids {
        let uuid = Uuid::parse_str(&id)?;
        assert_eq!(uuid.get_version_num(), 7);
        assert_eq!(uuid.get_variant(), uuid::Variant::RFC4122);
        assert!(uuid.get_timestamp().is_some());
    }
    Ok(())
}
