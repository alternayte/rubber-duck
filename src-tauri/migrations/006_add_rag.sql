CREATE TABLE code_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    content TEXT NOT NULL,
    language TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_code_chunks_repo ON code_chunks(repo_id);

CREATE VIRTUAL TABLE code_chunks_fts USING fts5(
    content,
    content='code_chunks',
    content_rowid='id'
);

CREATE TRIGGER code_chunks_ai AFTER INSERT ON code_chunks BEGIN
    INSERT INTO code_chunks_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER code_chunks_ad AFTER DELETE ON code_chunks BEGIN
    INSERT INTO code_chunks_fts(code_chunks_fts, rowid, content) VALUES('delete', old.id, old.content);
END;

CREATE VIRTUAL TABLE IF NOT EXISTS code_chunks_vec USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding float[384]
);
