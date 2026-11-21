use rusqlite::{Connection, ErrorCode};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn cli(config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tp"));
    command.arg("--config").arg(config);
    command
}

#[test]
fn installed_cli_runs_from_another_directory() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let portable = temp.path().join("portable");
    let unrelated = temp.path().join("another folder");
    fs::create_dir(&portable)?;
    fs::create_dir(&unrelated)?;
    fs::create_dir(portable.join("fixtures"))?;
    let binary = portable.join(if cfg!(windows) { "tp.exe" } else { "tp" });
    fs::copy(env!("CARGO_BIN_EXE_tp"), &binary)?;
    fs::write(
        portable.join("config.yaml"),
        "database_path: data/tp.sqlite\nscan_roots: [fixtures]\n",
    )?;
    fs::write(portable.join("fixtures/note.md"), "Disposable fixture")?;
    let run = |args: &[&str]| -> anyhow::Result<String> {
        let result = Command::new(&binary)
            .current_dir(&unrelated)
            .env_remove("PATH")
            .args(args)
            .output()?;
        anyhow::ensure!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Ok(String::from_utf8(result.stdout)?)
    };
    let receipt: serde_json::Value = serde_json::from_str(&run(&["scan"])?)?;
    assert_eq!(receipt["files"], 1);
    let note = portable.join("fixtures/note.md");
    let uri = run(&["link", "--no-clipboard", note.to_str().unwrap()])?;
    let id = uuid::Uuid::parse_str(uri.trim().strip_prefix("tp://").unwrap())?;
    assert_eq!(id.get_version_num(), 7);
    Ok(())
}

#[test]
fn interrupted_scan_does_not_mark_missing() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("fixtures");
    fs::create_dir(&root)?;
    let config = temp.path().join("config.yaml");
    fs::write(
        &config,
        "database_path: tp.sqlite\nscan_roots: [fixtures]\n",
    )?;
    let existing = root.join("previous.txt");
    fs::write(&existing, "indexed file")?;
    let initial = cli(&config).arg("scan").output()?;
    anyhow::ensure!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    fs::remove_file(existing)?;
    // A sparse file provides time to see the scanner's write transaction during traversal.
    fs::File::create(root.join("slow.bin"))?.set_len(512 * 1024 * 1024)?;
    let reader = Connection::open(temp.path().join("tp.sqlite"))?;
    reader.busy_timeout(Duration::ZERO)?;
    let mut child = cli(&config)
        .args(["scan", "--full-hash"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut locked = false;
    while Instant::now() < deadline && child.try_wait()?.is_none() {
        match reader.execute_batch("BEGIN IMMEDIATE") {
            Ok(()) => reader.execute_batch("ROLLBACK")?,
            Err(error) if error.sqlite_error_code() == Some(ErrorCode::DatabaseBusy) => {
                locked = true;
                break;
            }
            Err(error) => return Err(error.into()),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let kill = child.kill();
    child.wait()?;
    assert!(locked, "the scan did not hold a write transaction");
    kill?;
    let status: String = reader.query_row(
        "SELECT status FROM locations WHERE path LIKE '%previous.txt' AND is_current=1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(status, "active");
    Ok(())
}

#[cfg(unix)]
#[test]
fn unreadable_subtree_is_unknown() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir()?;
    let root = temp.path().join("fixtures");
    let denied = root.join("denied");
    fs::create_dir_all(&denied)?;
    fs::write(denied.join("record.txt"), "indexed file")?;
    let config = temp.path().join("config.yaml");
    fs::write(
        &config,
        "database_path: tp.sqlite\nscan_roots: [fixtures]\n",
    )?;
    let initial = cli(&config).arg("scan").output()?;
    assert!(initial.status.success());
    let permissions = fs::metadata(&denied)?.permissions();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0))?;
    let denied_effectively =
        fs::read_dir(&denied).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    let result = cli(&config).arg("scan").output();
    fs::set_permissions(&denied, permissions)?;
    assert!(result?.status.success());
    let db = Connection::open(temp.path().join("tp.sqlite"))?;
    let status: String = db.query_row(
        "SELECT status FROM locations WHERE path LIKE '%record.txt' AND is_current=1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        status,
        if denied_effectively {
            "unknown"
        } else {
            "active"
        }
    );
    Ok(())
}
