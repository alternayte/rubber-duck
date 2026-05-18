CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
    content,
    conversation_id UNINDEXED,
    session_id UNINDEXED
);

INSERT INTO conversations_fts (content, conversation_id, session_id)
SELECT content, id, session_id FROM conversations;

CREATE TRIGGER conversations_fts_insert AFTER INSERT ON conversations
BEGIN
    INSERT INTO conversations_fts (content, conversation_id, session_id)
    VALUES (NEW.content, NEW.id, NEW.session_id);
END;

CREATE TRIGGER conversations_fts_delete AFTER DELETE ON conversations
BEGIN
    DELETE FROM conversations_fts WHERE conversation_id = OLD.id;
END;
