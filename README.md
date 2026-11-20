# EZRA

Title
Linked Records — Beginning to End

Version
0.1

Date
2026-11-20

Authors
David Lowe (vision, rulings), Claude (walkthrough). Builds on Codex's Canonical Filing and Tagging Standard v0.4, kept intact as the reference.

Status
working guide, for the crew and for research

Linked Records: Beginning to End
This is one walk through the whole system, from a file arriving to a finished report. Two documents feed it:

The Linked Record System v0.1 (the idea)
Codex's Canonical Filing and Tagging Standard v0.4 (the detailed rules)
Codex's paper stays as written, as the reference. Where this guide says "see §N," that's Codex §N.

Rule 0: nobody moves your originals
No AI or program moves, renames or rearranges an original document. Order lives in labels and views. Moving a file is something David does himself. (David's ruling, Nov 20, 2026. It sits above everything else.)

Step 1: A file arrives
You drop something in an inbox: a paper, a zip, a transcript, a folder from an old clone. Nothing happens to it. It's recorded as it is, with where it came from and when. (Codex §14: capture is low-friction, keep the original intact.)

Step 2: It becomes a record
The scanner sees the file and gives it an entry in the index, the behind-the-scenes database. The entry holds:

a permanent record ID,
a fingerprint (sha256), so it can be recognized if moved,
its current location,
dates first and last seen.
Markdown and HTML also get the ID written into their header, so it travels with the file. Everything else is tracked by fingerprint. The file itself is untouched except for that one header line. (Paper §3. Codex §3 on identity: keep existing IDs, never mint over them.)

What does this today: D:\Schema_Engine\metadata_db.py + filetagger_core.py, minus the ID column.

Step 3: Ask the six questions
Every record gets labeled by answering, in order (Codex "Start here"):

Series. Which series owns it? (MIM, FP, God IS, Thirteen Roads…)
Paper. Which paper, and which edition?
Slot. Which of the 21 jobs is it doing? (INB, REV, STR, CAN, SYN, CAS, HIS, MTH, THE, TRM, RES, WRT, RDR, STQ, ORB, MED, RCP, PAU, PUB, GAP, SUP)
Item. What kind of thing within that job? (transcript, objection, derivation, API receipt…)
Connections. What does it link to? (sources, the Master Equation, a Law, an axiom, another series)
Standing. How far along is it? Kept as separate fields: processed, evidence, proof, Lean, canon, published. One never implies another. (Codex §12)
Unknown answers stay unknown. Nothing is guessed to fill a blank.

Who answers:

Location rules answer most of it for free: a file inside MIM.06_… is series MIM, paper MIM.06; a file in .BACKSIDE\api is RCP.
The classifier proposes what the rules can't (Codex §20 prompt).
David or the crew confirm the doubtful ones and the real papers.
Series or piece of a series?
A series (a set of papers that make one argument, e.g. FP's 7 papers) gets a series record and the 21 slots. The slots are logical. A folder is made only when something goes in it.
Everything else (a report, a run, a single fragment, a script) is a record that belongs to something: series + slot. It doesn't get a home of its own.
No home yet? series: unassigned. It waits in a holding view.
Collections are different from series
A collection (like AXIOM) is several series, versions and dead ends piled together. It needs one extra step first: separate the families/versions (e.g. 188 technical, 22 public, chain, Diamond, God IS), date them, and mark each one current, superseded or outdated. Then pick the keeper, and the keeper becomes the series. The rest stay linked as history (SUP/HIS).

Tools and scripts
Scripts that support the work (scanners, builders, runners) are not content. They belong to a system/tools home (the STR slot of the tooling itself), not inside a content series. Their outputs (reports, inventories) are records in the RCP slot of whatever series they ran on.

Step 4: Link it in four directions
Every record sits in the middle of four directions (David's ruling):

Direction	Means	Examples
UP	Labels about it: its metadata	series, slot, type, category, Law, date, author, grade, status
DOWN	What's inside or underneath	files, evidence, math, Lean receipts, API runs, dependencies
LEFT	Where it came from	question it answers, earlier versions, history
RIGHT	Where it goes	next question, bridges, witnesses, Scripture
The links themselves carry details: type (depends_on, supports, version_of… Codex §10 has 13), who proposed it, who reviewed it, status (proposed / reviewed / ruled). A bridge gets a grade only when earned: ISO, PART, ANLG or WIT (Codex §11).

(Codex §18 currently says UP = foundations. That changes to UP = metadata.)

Step 5: The index is the boss
All labels and links live in one database. Five tables:

Table	Holds
records	one row per thing: ID, series, slot, title, type
labels	any number of name/value labels per record (this is UP; add new kinds any time)
links	from, to, type, grade, status, proposer, reviewer (LEFT/RIGHT)
locations	path, fingerprint, last seen (survives moves)
recipes	saved report definitions
Start in SQLite on the local PC with one writer. Move to Postgres on the NAS when several AIs write at once. Don't run SQLite on the NAS share, because its locking isn't reliable over SMB.

A rescan keeps it honest: it re-finds moved files by ID or fingerprint, and reports anything it can't match. It never guesses.

Step 6: Look at it
Everything below reads from the index. Nothing is hand-edited, and nothing is copied.

Down one series: MIM → 21 slots → papers → files. (Like Snap2HTML, but built from labels.)
Across every series: pick a slot, e.g. "All ORB" or "All operators," and get it from every series side by side. These are the disciplined folders: links, not copies.
Four-direction map: Freeplane, generated per paper. Working now: make_freeplane_map.py → MIM.06.ACTS.mm, 68 nodes, 27 links, opened Nov 20.
Clickable names everywhere: a short label like Core Canon Papers in Markdown, HTML or Word, with the full path on hover. A nerve hotkey covers plain text.
Step 7: Recipes make reports
A recipe is a small YAML file:

yaml
view:   All ORBITS
pull:   slot = ORB
group:  [category: God/Spiritual, then Agents, then everything else]
sort:   date, newest → oldest
show:   title, series, paper, date, link
output: markdown + html
Run it and the report comes out, every line linked back to the original. Missing labels land in an unlabeled bucket at the bottom.

Step 8: Review and keep honest
AI Crew reviews sit right after Inbox (REV). A checkmark means "read this version," not "agreed" (Codex §13).
Issues are never erased. Fixes are appended as events.
Private audit (PAU) never goes into public exports. NAS paths are stripped from anything shared (Codex §14).
A label describes; it doesn't rule. "CAS" or "ISO" on a record is a claim to check.
Step 9: Publish
Edit the owned Markdown, then generate the HTML from it, with a build manifest naming the exact source (Codex §1, §5). Canon and publication are separate decisions with their own records (Codex §12).

What's working, what's next
Working now:

The 21 slots and their codes.
The four-direction map generator.
The index and scanner base in Schema_Engine.
Codex's full standard.
Crew prompts for the AXIOM cleanup.
Build order:

Add record_id / series / slot columns. Dry-run the scan on MIM (read-only).
ID-in-header anchor plus the rescan reconciler.
Recipe runner: "All ORBITS" and "All Operators" on MIM.
Slot-picker page (Snap2HTML style, built from the index).
Name registry and linker; nerve hotkey.
AXIOM: separate the families, pick God IS as the keeper, build the series.
Open rulings (David): R0 identity convention, R1 API date format, R2 parent-layer name, R3 bridge ownership, R4 Scripture location, R5 numbering. Plus CAS granularity. (Codex §15)




# Build brief for Codex: `tp://` permanent links for David's whole corpus

**From:** David Lowe (via Claude), Nov 20, 2026
**Build:** from zero, as a standalone, portable repo.
**Background reading (optional):** `LINKED_RECORD_SYSTEM_v0.1.md` and `WALKTHROUGH_BEGINNING_TO_END_v0.1.md` in `\\192.168.2.50\h_hp\Obsidian\00_APPS\00_SCHEMA_BUILD\`, and your own Filing and Tagging Standard v0.4.

## What it is

Every file and folder in David's corpus gets a **permanent ID**. A link of the form `tp://<ID>` opens that item from anywhere links work: Word, Excel, HTML, Markdown, Obsidian, PDF, Freeplane, browsers. **It keeps working after the item is moved or renamed,** because the link holds the ID and a local index holds the current location. Windows doesn't do this natively. This repo adds it.

## Hard rules

0. **Never move, rename, delete or rearrange any original file.** (David's standing rule.) The tool only reads files and writes to its own database and output folders.
1. **Never edit file contents by default.** Writing an ID into a Markdown/HTML header is an *opt-in* flag (`--write-ids`), off by default, and even then it only adds one frontmatter line and never touches anything else.
2. **Portable.** It must work from any folder after a GitHub download. No hard-coded paths. Config goes in a `config.yaml` next to the code. Python 3.11+, standard library preferred. Any dependency must be pip-installable and listed in `requirements.txt`.
3. **Local SQLite only**, with the database on the local disk. Refuse to open the database on a network share (SMB locking is unsafe), with a clear message.
4. **Network paths:** read scan roots via UNC (`\\192.168.2.50\h_hp\…`), never mapped drive letters. Python stalls on David's `H:` letter but works on UNC. Accept drive letters in config and translate them to UNC where possible.
5. **Unknown stays unknown.** Never guess a match. Anything ambiguous goes to a "needs review" list.

## Components

### 1. Index (`tp_index.sqlite`)
Tables:
- **records:** `id` (UUIDv7), `kind` (file/folder/view), `title`, `created_at`
- **locations:** `record_id`, `path`, `sha256`, `size`, `mtime`, `first_seen`, `last_seen`, `status` (active/missing)
- **aliases:** `record_id`, `alias` (short names like "Core Canon Papers", existing IDs like `MIM.06.ACTS`, filenames), `source`
- **labels:** `record_id`, `name`, `value` (open-ended, for series, slot, category etc.; filled later by the χ classifier, not this tool)
- **links:** `from_id`, `to_id`, `type`, `status`, `proposed_by` (schema only for now)

### 2. Scanner (`tp scan <root…>`)
- Walks roots incrementally: skip unchanged files by size+mtime, hash new or changed ones (sha256; a quick-hash option for files over 200 MB).
- Skips `.git`, `node_modules`, `.lake`, `__pycache__`, `.obsidian`, plus anything in a configurable skip list.
- **Identity:**
  - If a Markdown/HTML file already has `record_id:` or `uuid:` in its frontmatter, **use it**.
  - Otherwise, an existing series ID in the name (pattern `SERIES.NN.SLUG`, e.g. `MIM.06.ACTS`) is stored as an alias.
  - New files get a fresh UUIDv7.
- **Moves:** a file missing from its old path that shows up elsewhere with the same sha256 (or the same header ID) keeps its record ID. Only its location changes.
- **Ambiguity:** if one hash matches several missing records, don't guess. Write it to `needs_review.csv`.
- Writes a scan receipt: counts of new, unchanged, moved, missing and ambiguous.

### 3. Resolver (`tp open <link>`) + protocol handler
- Registers `tp://` under `HKCU\Software\Classes\tp`, so no admin is needed. Commands: `tp register` / `tp unregister`.
- `tp://<ID-or-alias>` opens the item at its current location (`os.startfile`).
- Options:
  - `?folder` opens the containing folder with the item selected (`explorer /select,`)
  - `?map` opens a generated four-direction map if one exists (see 5)
  - `?info` opens a small HTML card: ID, aliases, all known locations, labels, last seen
- **Missing item:** show the info card with "missing since <date>" and the last known path. Never fail silently.
- **Ambiguous alias:** show a chooser page listing every match.

### 4. Link helpers
- `tp link <path>` prints the `tp://` link for a file (and copies it to the clipboard).
- An Explorer right-click entry, "Copy tp link" (HKCU, opt-in via `tp register --context-menu`).
- `tp alias add <ID> "<short name>"` adds a short name.

### 5. Linker (`tp linkify <file> --out <dir>`)
- For Markdown and HTML: finds known aliases and raw paths that are in the index, and rewrites them as links in a **copy** written to `--out`. **Never overwrites the original.**
  - Markdown: `[Short Name](tp://ID "full current path")`
  - HTML: `<a href="tp://ID" title="full current path">Short Name</a>`
- DOCX (stretch goal): the same thing via python-docx, written to a new file.

### 6. Views (stretch goal, after 1–5 work)
- `tp view --label slot=ORB` writes an HTML page listing matching records as `tp://` links.
- `tp map <ID>` writes a Freeplane `.mm` with UP (labels), DOWN (contents/links out), LEFT (history), RIGHT (bridges). A reference generator already exists at `\\192.168.2.50\h_hp\Obsidian\00_APPS\00_SCHEMA_BUILD\FREEPLANE_TEST\make_freeplane_map.py`.

## Acceptance tests (must pass, with a test script in the repo)

1. Scan a temp folder of 20 mixed files (md, html, pdf, png, zip, folder). Every file gets a record and an ID.
2. `tp link` on a file, then click or `tp open` the link: the file opens.
3. **Move and rename** a file inside the test folder, rescan, and open the same old link: it opens at the new place.
4. Delete a file and rescan: the link opens an info card saying "missing since …" with the last path.
5. Two identical copies moved at once: the result goes to `needs_review.csv`, with no guess.
6. `tp linkify` on a Markdown file containing a known alias and a raw path: the copy has `tp://` links, and the original is byte-identical before and after.
7. A database path on a UNC share is refused with a clear message.
8. `tp register` → a `tp://` link in a browser or Word opens the item. `tp unregister` removes it cleanly.
9. A full scan of `\\192.168.2.50\h_hp\Obsidian\MIM` (≈73,000 files) completes, and the scan receipt reports counts. Report run time.

## Deliverables
- The repo: README with install, `tp register`, scan and usage; `config.example.yaml`; tests.
- A short report: what works, what doesn't, timings from test 9, known limits.
- Post a summary to crew comms (`all`) when done.

## Out of scope for this build
Classification (the χ classifier fills `labels` later), recipe reports, the nerve hotkey, and Postgres. Keep the schema ready for them, but don't build them.
