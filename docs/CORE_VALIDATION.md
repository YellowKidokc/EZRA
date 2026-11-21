# EZRA core validation

The Rust CLI builds from the locked dependencies with bundled SQLite. The
Linux test suite uses temporary synthetic files. Windows-specific test
binaries are compiled separately. The repository references and tests can
be used without a NAS connection.

## Acceptance results

| Check | Result |
| --- | --- |
| 1–6 | Passed on Linux. Mixed inventory, mocked file opening, move and delete reconciliation, duplicate-content holds, and copy-only Markdown/HTML linkification. |
| 7 | Local path, UNC rejection, and rejection of an unrelated SQLite database passed. Mapped network-drive behavior remains a Windows target-machine check. |
| 8 | URI parsing and executable-only handler command formation passed. Windows registry and browser/Word click-through remain open. |
| 9 | Real MIM scan remains open. The synthetic run below used disposable files. |
| 10–18 | Passed on Linux. Includes revision-scoped proof receipts, excluded/offline/denied coverage, an interrupted scan, identity conflicts, opt-in byte-preserving inserts, full-hash comparisons, URI encoding, and relocation limits. |
| 19 | Run `cargo test --locked` in a fresh checkout. The necessary references, configuration example, and synthetic fixtures are in the repository. |
| 20 | Passed with six concurrent callers. Verified 186 serialized, unique, ordered UUIDv7 IDs and RFC variant bits. |
| 21 | Windows x86-64 GNU release cross-build and PE dependency inspection passed. The executable imports only Windows system libraries. The Linux binary passed a CLI smoke test copied to another folder and invoked without `PATH`. Native Windows launch on a clean system remains open. |

Run `cargo test --locked` from the checkout for the automated suite. The
Windows build uses static CRT linkage with the MSVC target. For the GNU
cross-build, use an x86-64 MinGW-w64 C compiler, binutils and Rust target
`x86_64-pc-windows-gnu`.

## Synthetic scale run

The Linux release scanned 73,254 small fixture files across 1,858 directories,
including the requested root. All three runs reported complete coverage and
zero errors, conflicts, or holds.

| Run | Observations | Scanner time |
| --- | ---: | ---: |
| Initial scan | 75,112 new | 3.126 s |
| Incremental scan | 75,112 unchanged | 2.534 s |
| Full-hash audit | 75,112 unchanged | 2.602 s |

Linkification also passed against the populated index. The fixture files were small and local. The HP's real MIM scan will measure
the time and coverage of the actual corpus and network share.

## Run on the HP

Use a stable folder containing `tp.exe` and its `config.yaml`. Do the
protocol checks against a disposable local folder first.

1. Run `tp.exe --version` and `tp.exe scan` from another working directory
   on Windows without Rust, Python, or the Visual C++ developer tools.
   Confirm the scan uses the config beside the executable.
2. Configure a disposable mapped network drive as the database location.
   Verify that the same command rejects it. Use a local database for the
   remaining tests. If testing from a Rust checkout on the HP, set
   `EZRA_TEST_MAPPED_ROOT` to the mapped drive root and run
   `cargo test --locked check_07_local_database_gate`.
3. Scan one disposable local file, run `tp.exe link <file>`, then run
   `tp.exe register`. Use the printed link in a clickable HTML anchor and
   a Word hyperlink. Record app versions, prompts, and the file that
   opens. Test `?info` and `?folder` separately. If testing the Explorer
   menu, register again with `--context-menu` and verify it copies the URI.
   Run `tp.exe unregister` at the end.
4. Set up a separate local database and output directory for the MIM pilot.
   Set the scan root to the reachable UNC path. Run `tp.exe scan --full-hash`
   without `--write-ids`. Retain `scan-receipt.json` and `needs_review.csv`
   locally. Report elapsed time, files, directories including the requested
   root, completed and unavailable coverage, exclusions, and conflicts.

## Implementation limits

A folder renamed outside EZRA receives a different ID at its new path.
A new unanchored path is held for review if a freshly absent file could
have been edited and moved into it. A full hash can establish an
unambiguous unchanged move within scanned coverage. An existing supported
UUID anchor can establish identity across an edit and move. Existing
disputes may require review when coverage changes.

Header insertion supports UTF-8 Markdown frontmatter and HTML with a
recognized head. Malformed identity metadata stays on review. The scanner
checks for source changes before insertion, but opt-in source edits and
SQLite updates cannot commit atomically together. Keep `--write-ids` off
for the real pilot and avoid concurrent editing when testing it.

Map lookup can open an existing file identified by a reviewed or ruled
`map_path` label for the record. The core has no map generator. Linkification
targets plain text in supported Markdown and HTML and reports ambiguous
terms. It skips several common markup constructs and preserves the input
file. DOCX processing, the NAS watcher, and snapshot restore remain deferred.
