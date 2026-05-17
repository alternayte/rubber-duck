CREATE TABLE repos (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    local_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
