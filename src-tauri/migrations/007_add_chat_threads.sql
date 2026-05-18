CREATE TABLE chat_threads (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT 'Chat 1',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_chat_threads_session ON chat_threads(session_id);

ALTER TABLE conversations ADD COLUMN thread_id TEXT REFERENCES chat_threads(id) ON DELETE CASCADE;

-- Migrate: create one thread per session that has messages
INSERT INTO chat_threads (id, session_id, title, created_at)
SELECT
    'thread-' || s.id,
    s.id,
    'Chat 1',
    s.created_at
FROM sessions s
WHERE EXISTS (SELECT 1 FROM conversations c WHERE c.session_id = s.id);

-- Assign existing messages to their session's default thread
UPDATE conversations SET thread_id = 'thread-' || session_id
WHERE thread_id IS NULL;
