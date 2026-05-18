use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::{ChatThread, ConversationMessage};

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<ConversationMessage> {
    Ok(ConversationMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        rag_context: row.get(4)?,
    })
}

fn row_to_thread(row: &rusqlite::Row) -> rusqlite::Result<ChatThread> {
    Ok(ChatThread {
        id: row.get(0)?,
        session_id: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

// --- Thread CRUD ---

pub fn create_thread(conn: &Connection, session_id: &str, title: &str) -> AppResult<ChatThread> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO chat_threads (id, session_id, title) VALUES (?1, ?2, ?3)",
        params![id, session_id, title],
    )?;
    get_thread(conn, &id)
}

pub fn get_thread(conn: &Connection, thread_id: &str) -> AppResult<ChatThread> {
    conn.query_row(
        "SELECT id, session_id, title, created_at, updated_at FROM chat_threads WHERE id = ?1",
        params![thread_id],
        row_to_thread,
    )
    .map_err(|e| crate::error::AppError::Db(e))
}

pub fn list_threads(conn: &Connection, session_id: &str) -> AppResult<Vec<ChatThread>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, title, created_at, updated_at
         FROM chat_threads WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;
    let threads = stmt
        .query_map(params![session_id], row_to_thread)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(threads)
}

pub fn rename_thread(conn: &Connection, thread_id: &str, title: &str) -> AppResult<ChatThread> {
    conn.execute(
        "UPDATE chat_threads SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![title, thread_id],
    )?;
    get_thread(conn, thread_id)
}

pub fn delete_thread(conn: &Connection, thread_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM chat_threads WHERE id = ?1", params![thread_id])?;
    Ok(())
}

// --- Message functions ---

pub fn save_message(conn: &Connection, session_id: &str, thread_id: &str, role: &str, content: &str) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, thread_id, role, content) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_id, thread_id, role, content],
    )?;
    Ok(())
}

pub fn save_message_with_context(
    conn: &Connection,
    session_id: &str,
    thread_id: &str,
    role: &str,
    content: &str,
    rag_context: Option<&str>,
) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, thread_id, role, content, rag_context) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, session_id, thread_id, role, content, rag_context],
    )?;
    Ok(())
}

pub fn list_by_session(conn: &Connection, session_id: &str) -> AppResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at, rag_context
         FROM conversations WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;
    let messages = stmt
        .query_map(params![session_id], row_to_message)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}

pub fn list_by_thread(conn: &Connection, thread_id: &str) -> AppResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at, rag_context
         FROM conversations WHERE thread_id = ?1
         ORDER BY created_at ASC",
    )?;
    let messages = stmt
        .query_map(params![thread_id], row_to_message)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}

pub fn delete_from_message(conn: &Connection, thread_id: &str, message_id: &str) -> AppResult<usize> {
    let pivot_rowid: i64 = conn
        .query_row(
            "SELECT rowid FROM conversations WHERE id = ?1 AND thread_id = ?2",
            params![message_id, thread_id],
            |row| row.get(0),
        )
        .map_err(|_| crate::error::AppError::Other(format!("Message {message_id} not found")))?;

    let deleted = conn.execute(
        "DELETE FROM conversations WHERE thread_id = ?1 AND rowid >= ?2",
        params![thread_id, pivot_rowid],
    )?;

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn setup_session_and_thread(conn: &Connection) -> String {
        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();
        let thread = create_thread(conn, "s1", "Chat 1").unwrap();
        thread.id
    }

    #[test]
    fn save_and_list() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let thread_id = setup_session_and_thread(&conn);

        save_message(&conn, "s1", &thread_id, "User", "Hello duck").unwrap();
        save_message(&conn, "s1", &thread_id, "Assistant", "How can I help?").unwrap();

        let messages = list_by_thread(&conn, &thread_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "User");
        assert_eq!(messages[0].content, "Hello duck");
        assert_eq!(messages[1].role, "Assistant");
        assert_eq!(messages[1].content, "How can I help?");
    }

    #[test]
    fn list_empty_thread() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let thread_id = setup_session_and_thread(&conn);

        let messages = list_by_thread(&conn, &thread_id).unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn delete_from_message_removes_target_and_later() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let thread_id = setup_session_and_thread(&conn);

        save_message(&conn, "s1", &thread_id, "User", "First").unwrap();
        save_message(&conn, "s1", &thread_id, "Assistant", "Response 1").unwrap();
        save_message(&conn, "s1", &thread_id, "User", "Second").unwrap();
        save_message(&conn, "s1", &thread_id, "Assistant", "Response 2").unwrap();

        let messages = list_by_thread(&conn, &thread_id).unwrap();
        assert_eq!(messages.len(), 4);

        let target_id = &messages[2].id.clone();
        let deleted = delete_from_message(&conn, &thread_id, target_id).unwrap();
        assert_eq!(deleted, 2);

        let remaining = list_by_thread(&conn, &thread_id).unwrap();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].content, "First");
        assert_eq!(remaining[1].content, "Response 1");
    }

    #[test]
    fn delete_from_message_nonexistent_returns_error() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let thread_id = setup_session_and_thread(&conn);

        let result = delete_from_message(&conn, &thread_id, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn create_and_list_threads() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        let t1 = create_thread(&conn, "s1", "Chat 1").unwrap();
        let t2 = create_thread(&conn, "s1", "Chat 2").unwrap();

        let threads = list_threads(&conn, "s1").unwrap();
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, t1.id);
        assert_eq!(threads[1].id, t2.id);
        assert_eq!(threads[1].title, "Chat 2");
    }

    #[test]
    fn rename_thread_updates_title() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        let thread = create_thread(&conn, "s1", "Original").unwrap();
        let updated = rename_thread(&conn, &thread.id, "Renamed").unwrap();
        assert_eq!(updated.title, "Renamed");
    }

    #[test]
    fn delete_thread_cascades_messages() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let thread_id = setup_session_and_thread(&conn);

        save_message(&conn, "s1", &thread_id, "User", "Hello").unwrap();

        delete_thread(&conn, &thread_id).unwrap();

        let msgs: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(msgs, 0);
    }
}
