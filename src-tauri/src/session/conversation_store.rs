use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::ConversationMessage;

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<ConversationMessage> {
    Ok(ConversationMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
    })
}

pub fn save_message(conn: &Connection, session_id: &str, role: &str, content: &str) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, ?3, ?4)",
        params![id, session_id, role, content],
    )?;
    Ok(())
}

pub fn list_by_session(conn: &Connection, session_id: &str) -> AppResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at
         FROM conversations WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;
    let messages = stmt
        .query_map(params![session_id], row_to_message)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn save_and_list() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        save_message(&conn, "s1", "User", "Hello duck").unwrap();
        save_message(&conn, "s1", "Assistant", "How can I help?").unwrap();

        let messages = list_by_session(&conn, "s1").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "User");
        assert_eq!(messages[0].content, "Hello duck");
        assert_eq!(messages[1].role, "Assistant");
        assert_eq!(messages[1].content, "How can I help?");
    }

    #[test]
    fn list_empty_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        let messages = list_by_session(&conn, "s1").unwrap();
        assert_eq!(messages.len(), 0);
    }
}
