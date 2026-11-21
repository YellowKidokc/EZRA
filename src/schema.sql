CREATE TABLE IF NOT EXISTS records (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('file','folder','view')),
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS labels (
    record_id TEXT NOT NULL REFERENCES records(id),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL DEFAULT 'text',
    source TEXT NOT NULL,
    review_status TEXT NOT NULL,
    PRIMARY KEY (record_id,name,value,source)
);
CREATE TABLE IF NOT EXISTS links (
    from_id TEXT NOT NULL REFERENCES records(id),
    to_id TEXT NOT NULL REFERENCES records(id),
    type TEXT NOT NULL,
    status TEXT NOT NULL,
    proposed_by TEXT,
    reviewed_by TEXT,
    grade TEXT,
    source_revision INTEGER,
    source_sha256 TEXT
);
CREATE TABLE IF NOT EXISTS locations (
    id INTEGER PRIMARY KEY,
    record_id TEXT REFERENCES records(id),
    claimed_id TEXT,
    path TEXT NOT NULL,
    sha256 TEXT,
    size INTEGER,
    mtime TEXT,
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    observed_at INTEGER NOT NULL,
    missing_since INTEGER,
    revision_seq INTEGER NOT NULL,
    is_current INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL CHECK (status IN ('active','missing','unknown','conflict','moved')),
    kind TEXT NOT NULL,
    reason TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS locations_current_path ON locations(path) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS locations_record ON locations(record_id, revision_seq);
CREATE TABLE IF NOT EXISTS aliases (
    record_id TEXT NOT NULL REFERENCES records(id),
    alias TEXT NOT NULL,
    normalized_lookup_key TEXT NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY (record_id,alias,source)
);
CREATE INDEX IF NOT EXISTS aliases_lookup ON aliases(normalized_lookup_key);
