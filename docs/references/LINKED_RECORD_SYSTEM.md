# Linked Records — portable reference

Adapted reference for EZRA, November 20, 2026. Based on David Lowe's direction and Claude's *The Linked Record System v0.1* and walkthrough. This is a self-contained summary, not a verbatim archival copy or a statement that the whole architecture is implemented.

## One record, several views

A paper, source, media item, folder, tool output or relationship has an identity separate from its address. Files stay in their owned locations. Views gather references without making separately maintained content copies.

Within a series, readers can navigate papers and supporting material. Across series, readers can select a workspace role or subject and see related records together. A collection may contain several series and unresolved editions; it need not collapse into one keeper.

## Four directions

- UP: labels and metadata about a record.
- DOWN: contents and supporting records.
- LEFT: origins, versions, questions and history.
- RIGHT: outgoing connections, bridges and next questions.

These are display directions. They do not independently establish causation, proof or authority.

## What the index knows

The initial five-table core distinguishes records, labels, links, locations/revision observations and aliases. Content hashes identify bytes; paths locate objects; neither alone establishes all identity. Default scans do not write IDs into originals.

Same-path edits normally keep identity and append a revision. Missing checks apply only to successfully enumerated coverage. Duplicate IDs, ambiguous moves and possible replacements remain review questions. Folder movement and edited-plus-moved unanchored files have explicit v0.1 limits.

Proof/review receipts apply to exact content revisions. A link that still resolves does not make an old receipt apply to new content.

## Later views

Short clickable labels, Freeplane maps and YAML recipes can display the index. Generated displays are not hand-maintained source databases. A conceptual recipe can request all ORBITS records, group by subject, sort by date and show title/series/link. Its actual grammar and execution are deferred.

The Windows resolver and copy-only linkification are the first build; the complete worldview/graph application is not required to implement them.
