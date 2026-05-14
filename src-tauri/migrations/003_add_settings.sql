CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO settings (key, value, category) VALUES ('llm.model', 'deepseek/deepseek-chat-v4-0324:free', 'llm');
INSERT INTO settings (key, value, category) VALUES ('llm.api_key_ref', '', 'llm');
