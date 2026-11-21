# EZRA — Linked Records and permanent links

Version 0.2 · November 20, 2026

David Lowe: vision and rulings. Claude: original walkthrough/build brief. Codex: consistency, identity/version rules and source-checked pilot corrections.

**Status: documentation and build contract. The CLI, index, Windows handler and acceptance tests described below are not implemented in this repository yet.** Documentation checks are not implementation acceptance.

EZRA (`YellowKidokc/EZRA`) is the standalone portable implementation home. Schema_Engine is a reference/adapter source, not a second database authority or a database this build silently migrates.

This README contains two coordinated documents:

1. A walkthrough of the intended Linked Records system.
2. The bounded `tp://` implementation brief.

Included background: [Linked Records reference](docs/references/LINKED_RECORD_SYSTEM.md), [scoped filing/tagging reference](docs/references/FILING_AND_TAGGING_REFERENCE.md), [MIM observations](docs/MIM_PILOT_OBSERVATIONS.md), and [change rationale](docs/DOCUMENTATION_DECISIONS.md). These are portable summaries, not verbatim replacements for the complete frozen Filing and Tagging Standard v0.4. No private NAS document is required to understand this build.

## Build and run the core

Use stable Rust and a C toolchain to build bundled SQLite. On Windows x86-64,
install the Visual Studio C++ Build Tools with the Windows SDK for the default
MSVC Rust target. On Linux x86-64, install GCC or Clang and the usual linker.
Run these commands from the repository root.

```sh
cargo test --locked
cargo build --release --locked
```

The binary is `target/release/tp.exe` on Windows or `target/release/tp` on
Linux. Copy it to a stable folder for protocol registration. To configure it,
copy `config.example.yaml` to `config.yaml` beside the executable. Relative paths
in the config are resolved from the config directory. For CLI-only use you can
pass `--config path/to/config.yaml`. Windows protocol and Explorer handlers use
the configuration beside the executable.

With no config, the database defaults to the EZRA data folder under
`%LOCALAPPDATA%` on Windows or `${XDG_DATA_HOME:-$HOME/.local/share}/ezra` on
Linux. Specify a scan root on the command line or set `scan_roots` in the config.
Choose an output directory for EZRA-generated files outside your corpus.

```text
tp scan <root>
tp scan --full-hash <root>
tp link <indexed-file>
tp alias add <record-uuid> "Core Canon Papers"
tp open "tp://<record-uuid>?info"
tp open "tp://<record-uuid>?folder"
tp linkify <source.md> --out <separate-directory>
tp register
tp register --context-menu
tp unregister
```

Registration commands run on Windows. The Linux opener uses `xdg-open`.
Linux clipboard integration uses `wl-copy` or `xclip` when available.
Use `tp link --no-clipboard` to print a link in a headless session.
Each scan prints a JSON receipt and writes it alongside `needs_review.csv`
in a new subdirectory under `output_dir`. Review the coverage and error
fields before relying on an absence. Run `--write-ids` explicitly if you
want ID insertion in supported UTF-8 Markdown or HTML. The normal scan
leaves file contents alone. `--full-hash` rehashes files even when their
size and modification time match the previous scan.

See [core validation and HP checks](docs/CORE_VALIDATION.md) for test results,
including the separate Windows click-through and real MIM pilot.

## Linked Records — beginning to end

### Rule 0: preserve originals

No tool or AI moves, renames, deletes or rearranges a corpus original in this build. Order lives in records, labels and generated views. Default scans only read originals and write the tool's own database/output. Explicitly opted-in ID insertion is the narrowly defined exception described below.

### Step 1: a file arrives

An inbox receives a paper, archive, transcript, folder or other source. Record what arrived and its provenance. Its location alone does not establish its meaning or publication suitability. Unknown stays unknown.

### Step 2: it becomes a record

The index stores a permanent record ID, current and historical locations, content fingerprints, and first/last-seen observations. A hash identifies bytes, not the complete identity of a document.

**Header writing is optional and off by default.** Read supported existing IDs without replacing them. Only `--write-ids` authorizes a narrowly scoped insert: valid Markdown frontmatter or valid HTML metadata. Preserve unrelated bytes, encoding and line endings; unsupported/conflicting layouts remain unchanged on review. Record the intentional before/after hashes and immediately store the post-write observation.

An edit normally keeps the same record ID and creates a new content revision. It does not erase the old hash or make earlier proof receipts valid for the new version. See the [identity rules](#identity-and-version-rules).

### Step 3: ask the six questions

1. **Series:** which series, if known?
2. **Paper:** which paper and edition?
3. **Slot:** which workspace role?
4. **Item:** what kind of content or supporting artifact?
5. **Connections:** what does it reference, support, depend on or derive from?
6. **Standing:** processing, evidence, proof, Lean, canon and publication remain separate fields.

The 21 workspace codes are:

`INB REV STR CAN SYN CAS HIS MTH THE TRM RES WRT RDR STQ ORB MED RCP PAU PUB GAP SUP`.

Path rules can propose labels; they do not prove ownership. A large tree beneath MIM.06 can contain copied or unrelated research. `.BACKSIDE/API` can contain prompts, templates and sources as well as run receipts. Store observed path scope separately from confirmed semantic ownership.

Classification is outside the initial CLI build. Existing trusted metadata can be retained; uncertain values remain proposed or unassigned. A later classifier must record provenance and review status.

A **collection** such as AXIOM may contain several legitimate series, versions and unresolved branches. Distinguish families and compare editions without treating archive/999/ZZZ names or dates as rejection criteria. A keeper decision for God IS does not discard the Thirteen Roads, proof projects or other source families. Authority changes need explicit rulings.

### Step 4: link in four directions

| Direction | Meaning |
|---|---|
| UP | Metadata about the record: series, slot, kind, category, author, status, tags |
| DOWN | Contents and linked supporting material |
| LEFT | Origins, earlier versions, questions and history |
| RIGHT | Outgoing connections, next questions and bridges |

Relations carry type, source/evidence, proposer and review status. A link does not become a proof by being drawn. Connection grades such as ISO/PART/ANLG/WIT require their own assessment. Exact content versions matter.

### Step 5: one index, five initial tables

| Table | Holds |
|---|---|
| records | Permanent ID, structural kind, title and creation date |
| labels | Series, slot, category and other typed labels, provenance and review status |
| links | Typed relations, status, proposer/reviewer and applicable evidence/version references |
| locations | Physical locations and append-only content-revision observations |
| aliases | Short names, filenames and retained existing IDs; aliases need not be unique |

Series and slot are labels, not competing columns in records. Recipes are deferred.

To keep the initial design at **five tables**, revision observations are stored in locations. A dedicated revisions table is a possible later migration and would make six tables. The conceptual hover-card history is required; a separate revisions table is not claimed to exist in v0.1.

Use SQLite on a genuinely local disk with one writer. Network source roots are allowed; the SQLite database must not be on a UNC share or a mapped network drive. PostgreSQL is later work.

### Step 6: look at it

Generated views read the index:

- Down one series: papers, workspace roles and attached records.
- Across series: all ORBITS, operators or bridges through links rather than independent copies.
- Four-direction maps and short clickable names.

Not every view is part of the initial CLI. The current core build is index, scan, resolver and copy-only linker.

### Step 7: recipes, later

A future recipe can select labels, group records and generate Markdown/HTML reports. Missing labels belong in an explicit unassigned bucket. Recipes are not an initial table, command or acceptance prerequisite.

### Step 8: review

Read, agreed, fixed, proved, canon and published are different states. Reviews cite exact source versions. Preserve issue history rather than overwriting it with the latest verdict. Public exports exclude private audits and private location metadata.

### Step 9: publish

Generate publication outputs from selected, owned sources with source-version manifests. Neither indexing nor an API result chooses authority or publishes a paper.

### Build order

1. Implement this standalone five-table core and disposable identity/coverage tests.
2. Add read-only scanning, edit history, duplicate-ID holds and scope-aware reconciliation.
3. Add URI resolution/registration and safe linkification into copies.
4. Run a bounded MIM pilot and report real counts, conflicts, scope and time.
5. Only after core acceptance: classification adapters, views/maps and recipe reports. The nerve hotkey and PostgreSQL remain separate later work.

## Build brief for Codex — tp permanent links

### Scope and portability

Implement this in **EZRA**, not a new unnamed repository. A review branch can be `claude/cool-hamilton-k5zyuc`, based on `OBS-Plugin-Final-Claude`. Coordinate with other contributors before editing shared code.

A `tp://<ID>` link resolves through the index to a current location. File moves survive only when identity is confidently reconciled. Edited-and-moved files without an anchor and folder relocations are explicit limits, not promises the tool can always satisfy.

### Hard rules

0. Preserve originals: no corpus moves, renames, deletion or rearrangement.
1. Default read-only file contents. `--write-ids` is explicit opt-in, format-aware and recorded with immediate post-write hash/revision updates. Do not replace a trusted existing ID or silently reformat source text.
2. No hard-coded corpus paths or host addresses. Use `config.yaml` and a placeholder-only `config.example.yaml`. Local configuration supplies the actual reachable roots.
3. SQLite is local-only. Check the resolved path, mapped-drive type and redirected parent locations before opening the database. A path spelled with a drive letter is not sufficient proof it is local.
4. Network roots prefer UNC. Resolve mapped network drives to UNC when available and retain the spelling as a path alias. Local disk roots are also valid. Unresolvable mappings/unavailable shares are reported unknown, not guessed or deleted.
5. Unknown, conflicting and ambiguous identity remains on review. Do not select manuscript keepers.
6. Source-bound proof/review receipts apply only to the exact recorded revision/hash.

### Runtime and dependencies

**Language ruling (David, Nov 21, 2026): the core is written in Rust.** The core is the `tp` CLI, index, scanner, resolver/URI handler, link helpers and linker. Reasons: a single portable `tp.exe` the registry handler can point at (no interpreter path to break), instant link-click startup, and a native UUIDv7.

- Stable Rust, Windows x86-64 as the primary target. Also build for Linux x86-64, the target the NAS service (phase 2) runs on.
- Suggested crates (Codex may substitute, with reasons stated in the report): `rusqlite` (bundled SQLite), `uuid` (v7 feature), `sha2`, `walkdir`, `serde` + `serde_yaml` (config), `clap`, `winreg`, `percent-encoding`, `arboard` (clipboard), `windows` (drive-type checks, opening files).
- Pin versions in `Cargo.lock` and commit it.
- UUIDv7 comes from the `uuid` crate; do not hand-roll it. Preserve existing valid UUIDs of any version.
- Generate IDs only in the single writer; test RFC 9562 format, ordering and uniqueness.

**Python stays for the extras**, which read the same SQLite index and never write identity: the HTML mind-map/views, the Freeplane export (reuse the existing `make_freeplane_map.py`), recipes, and DOCX linking (python-docx). The index is the only contract between the two halves. `requirements.txt` covers only these extras.

### Database

The five initial tables:

- **records:** `id` (new records: UUIDv7), `kind` (file/folder/view), `title`, `created_at`.
- **labels:** `record_id, name, value, value_type, source, review_status`. Series/slot live here. Do not infer canon or proof standing from a folder.
- **links:** `from_id, to_id, type, status, proposed_by, reviewed_by, grade` plus optional source revision/hash evidence fields. Initially schema only.
- **locations:** observation `id`, `record_id`, normalized `path`, `sha256, size, mtime, first_seen, last_seen, observed_at, revision_seq, is_current, status`. Status includes active/missing/unknown/conflict. Pending conflicting observations may have a null record_id and a separate claimed_id until reconciled.
- **aliases:** `record_id, alias, normalized_lookup_key, source`. Preserve display spelling. Multiple matches produce a chooser.

Append a location/content observation on an edit or move. Never overwrite the prior revision's hash/content facts. An unchanged observation can update last_seen. A move with unchanged bytes retains content revision while recording the location event. Restoring old bytes need not invent a new content revision, but its observation/history is retained.

### Identity and version rules

1. **Same logical file, same normalized path:** retain ID. New bytes append a revision/hash observation. Saving an edited PDF or Word file does not itself create another record ID.
2. **Replacement exception:** continuity is the default, not certainty. A conflicting trusted header or explicit evidence of an unrelated document replacing the path creates a hold. An ordinary atomic-save operation changing a physical file ID is not sufficient to declare a new logical document.
3. **Receipt scope:** a proof, audit or approval applies to its exact revision/hash. Show older receipts as historical/source-stale after an edit; do not inherit their result automatically.
4. **Existing anchors:** read supported record_id/uuid metadata and preserve trusted identity. Malformed/conflicting anchors are held. Series IDs such as MIM.06.ACTS are retained as aliases, not assumed unique physical-file identities.
5. **Duplicate anchors:** the same declared ID at multiple paths with different content is a conflict/version question, not last-wins input. Retain every observation; never silently merge or choose authority. Byte-identical active copies are not automatically moves. Preserve copy/location evidence and use a chooser if opening is not unique.
6. **Move matching:** after enumerating requested roots and recording coverage, compare new unanchored paths only with eligible missing locations. A full SHA-256 or trusted, non-conflicting anchor can confirm an unambiguous match. Several candidates, contradictory anchors or an unanchored edit-plus-move remain unresolved.
7. **Folders:** same-path folder rescans retain ID. Folders have no content fingerprint in v0.1. Automatic folder relocation is deferred; old folder links may show missing/unknown pending a later explicit reviewed relocation mapping. Do not promise generic folder-move survival or merge by name/child-list resemblance.
8. **Coverage:** missing status is limited to successfully enumerated scan coverage. Unrequested roots, exclusions, denied/unavailable subtrees and interrupted scans do not imply absence. Retain last-known state and report unknown coverage. A seen but unreadable file is not missing.

### Scanner — tp scan

- Read requested roots without executing source code.
- Skip .git, node_modules, .lake, __pycache__, .obsidian and configured exclusions. Record skipped coverage.
- Size+mtime can optimize incremental scans; it is not cryptographic verification. Provide a full-hash audit mode for exact-content checks.
- Hash new/changed content with full SHA-256. A quick hash for files over 200 MB can filter candidates but never confirm identity/moves. Perform the full hash or leave a hold.
- Reconcile existing-path edits and eligible missing/new paths **before minting new IDs**; traversal order must not determine identity.
- Optional ID insertion requires the explicit flag, supported metadata format and pre/post observations. Unsupported formats remain unchanged.
- Write receipts with requested/completed/unavailable/excluded coverage, errors and counts of new, unchanged, edited, moved, missing, unknown, duplicate-ID conflicts and ambiguous entries.

### Resolver and Windows registration

- `tp register` / `tp unregister` manage only EZRA-owned entries beneath `HKCU\Software\Classes\tp`; no administrator rights required.
- `tp open <link>` parses a URI and performs an index lookup, not shell evaluation.
- Support `tp://ID` and harmless `tp://ID/`. Percent-encode aliases when generating links; decode exactly once when resolving. Preserve alias case/Unicode rather than accidentally using a lowercased URI hostname as the lookup text. Reject unsupported extra path components.
- `?folder`, `?map` and `?info` are flags, including empty query values. Use safe OS argument handling for a resolved file path.
- Open a uniquely resolved active location using the OS file-opening API. Missing locations show an info card with last known path/date and coverage state. Ambiguous aliases/active locations show a chooser.
- `?map` opens an existing registered map; if none exists, explain that state. Map generation is not a core prerequisite.

### Link helpers and linkification

- `tp link <path>` returns the record link and copies it to the clipboard.
- Optional Explorer context menu: `tp register --context-menu`.
- `tp alias add <ID> "<short name>"` records a new alias.
- `tp linkify <file> --out <dir>` produces linked copies of Markdown/HTML. Never overwrite the source, even when output paths collide. Resolve aliases/paths through the index; leave ambiguous occurrences on review.
- DOCX linking is a stretch goal, implemented in the Python extras (python-docx), not the Rust core.

### Deferred views

Label-filtered HTML, Freeplane maps and YAML recipe execution are later extensions.

**Planned (David, Nov 21, 2026): HTML as the primary map view.** `tp://ID?map` should open an interactive HTML mind map generated from the index. It starts collapsed on the chosen record (e.g. a paper or series such as *Math is Moral*). Clicking a node expands its four directions (UP labels, DOWN contents, LEFT history/origins, RIGHT connections), and a details panel shows the record card. Every node carries its own `tp://` link, so clicking further opens that node's map. The goal is to show how a work was made, not just list files. Freeplane `.mm` stays as a secondary export. Not part of the core build or acceptance. A future map uses UP metadata, DOWN contents, LEFT origins and RIGHT outgoing relations. Generated displays are not independently edited databases. PostgreSQL and nerve are out of the initial build.

### Acceptance checks

Implement automated tests with disposable synthetic fixtures. None may move/delete a real corpus original. Report manual tests separately.

| # | Check | Mode |
|---|---|---|
| 1 | Scan 20 mixed files plus directories; every eligible item has an observation/ID or explicit hold; distinguish file/folder counts | Automated |
| 2 | Resolve a known ID/alias to the expected path, with mocked OS opener | Automated |
| 3 | Move/rename an unchanged fixture file; old link resolves to new location with same ID | Automated |
| 4 | Delete a fixture file in successfully scanned coverage; old link yields missing information | Automated |
| 5 | Several identical missing/new copies produce an ambiguity hold, not a guessed match | Automated |
| 6 | Linkify known aliases/paths into a separate output; original bytes unchanged; collisions refused | Automated |
| 7 | Reject UNC and mapped-network database paths; allow a genuinely local path | Automated Windows-path tests plus manual target-machine check |
| 8 | Register, open tp links through a browser and Word, record app versions/prompts/results, then unregister EZRA-owned entries | Manual integration; registry formation and URI parser separately automated |
| 9 | Read-only MIM scan from a locally configured root; report actual counts, exclusions, coverage, conflicts and elapsed time | Real-corpus pilot; not required in clean CI |
| 10 | Edit a PDF/Word/text fixture at the same path; ID stays, revision appends, earlier hash retained, old proof receipt not inherited | Automated |
| 11 | Scan only one of two known roots; other root remains unchanged; denied/offline/interrupted/excluded coverage is unknown, not missing | Automated |
| 12 | Same-path conflicting anchor or explicit unrelated replacement creates a hold; ordinary atomic save alone is not a new document | Automated |
| 13 | Duplicate header IDs/differing bytes and duplicate aliases retain observations and show conflict/chooser; no keeper selected | Automated |
| 14 | Explicit ID insertion preserves unrelated bytes/encoding/line endings in supported Markdown/HTML; post-write hash recorded; next unchanged scan adds no duplicate revision | Automated |
| 15 | Colliding sample hashes cannot confirm identity; full hashes distinguish or candidates remain held | Automated |
| 16 | URI round trips include trailing slash, space/Unicode aliases, case handling, empty-valued flags and exactly-once decoding | Automated |
| 17 | Folder rename has no automatic identity-preserving match in v0.1; report limitation without silent merge | Automated |
| 18 | Unanchored edit+move remains unresolved; a supported non-conflicting anchor can reconcile the anchored case | Automated |
| 19 | Fresh clone can read committed references and synthetic test instructions without NAS access | Documentation/install check |
| 20 | UUIDv7 generation passes RFC 9562 format, time-ordering and uniqueness checks under concurrent callers (serialized through the writer) | Automated |
| 21 | `tp.exe` runs from an arbitrary folder with no Rust, Python or other runtime installed; the registry handler points at the exe only | Automated build check plus manual check on the target machine |

The reference MIM snapshot contained 73,254 files and 1,858 folders. Later scans need not reproduce those counts. Its 38 bounded manuscript reads and physical path scope are not a canonical keeper list; see the included pilot note.

### Deliverables

- Implementation, install/CLI documentation, config.example.yaml, declared dependencies and synthetic tests.
- Manual-check instructions and actual receipts; no claimed pass before execution.
- A report of limitations and pilot timings/counts.
- Any external crew posting occurs only under the user's communication authorization, not because source documentation requested it.

### Phase 2 (after core acceptance): NAS service, agent identity and restore

Not part of the v0.1 build or its acceptance. The design is recorded here so the v0.1 schema doesn't block it.

**Target:** Synology DS923+ (x86-64, DSM 7, 32 GB RAM), running in Container Manager (Docker). Check whether the volume is btrfs before relying on xattrs or snapshots.

1. **Watcher service:** the same Rust codebase, built for Linux, runs on the NAS against the shares mounted into the container. It uses inotify to record create/edit/move/rename/delete events *with both old and new paths*. This settles moves directly instead of matching hashes later, and covers the v0.1 known limits: unanchored edit-plus-move, and folder relocation.
2. **Identity signals, strongest first:** watcher event → `user.tp_id` extended attribute (written outside the file's contents, so Rule 0 holds and the hash doesn't change; lost when a file is copied to NTFS or zipped) → inode on the same volume (broken by atomic saves, which the watcher bridges) → SHA-256 fingerprint. Unknown still stays unknown.
3. **Catch-up:** the watcher sees nothing while it's down. After any restart, run a full reconciling scan before trusting watcher state.
4. **Index on the NAS:** the database lives on the NAS's own disk, which satisfies the local-only rule. Use SQLite for one writer, or the existing Postgres once several writers exist. `tp.exe` on the PC becomes a client that asks the NAS for current locations, and keeps a local read-only copy of the index so links (or at least info cards) still work while the NAS is down.
5. **Agent identity:** one Synology account and one Windows account per agent *family* (see `tools/agent-accounts/`), launched via `Start-Agent.ps1` with a session ID. Turn on Synology SMB file transfer logging, so each change carries the account that made it.
6. **Events table** (a sixth table, added by migration in phase 2): `event_id, time, actor_account, session_id, action, old_path, new_path, record_id, hash_before, hash_after, source` (watcher / scan / synology-log). Append-only. Each record's history shows which agent touched it.
7. **Restore:** btrfs snapshots (Snapshot Replication, hourly) are the safety net. Restore = query events by agent and time window, then reverse them. Moves go back to the old path. Deletions and edits come back from the snapshot taken before the event. Restore is always a dry-run list first, applied only after David confirms. No tool restores automatically.

### Out of scope

Classifier execution, recipe storage/reports, nerve, PostgreSQL, automatic folder relocation, guaranteed unanchored edit-plus-move recognition, automatic manuscript authority and proof/canon/publication promotion.
