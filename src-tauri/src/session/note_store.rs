use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::Note;

fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        session_id: row.get(1)?,
        content: row.get(2)?,
        sort_order: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub fn get_or_create(conn: &Connection, session_id: &str) -> AppResult<Note> {
    let existing = conn.query_row(
        "SELECT id, session_id, content, sort_order, created_at
         FROM notes WHERE session_id = ?1 ORDER BY sort_order ASC LIMIT 1",
        params![session_id],
        row_to_note,
    );

    match existing {
        Ok(note) => Ok(note),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO notes (id, session_id, content) VALUES (?1, ?2, '')",
                params![id, session_id],
            )?;
            let note = conn.query_row(
                "SELECT id, session_id, content, sort_order, created_at
                 FROM notes WHERE id = ?1",
                params![id],
                row_to_note,
            )?;
            Ok(note)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn update_content(conn: &Connection, id: &str, content: &str) -> AppResult<Note> {
    conn.execute(
        "UPDATE notes SET content = ?1 WHERE id = ?2",
        params![content, id],
    )?;
    conn.execute(
        "UPDATE sessions SET updated_at = datetime('now')
         WHERE id = (SELECT session_id FROM notes WHERE id = ?1)",
        params![id],
    )?;
    let note = conn.query_row(
        "SELECT id, session_id, content, sort_order, created_at
         FROM notes WHERE id = ?1",
        params![id],
        row_to_note,
    )?;
    Ok(note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::session::store as session_store;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn get_or_create_creates_on_first_call() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = session_store::create(&conn, "Test").unwrap();
        let note = get_or_create(&conn, &session.id).unwrap();

        assert_eq!(note.session_id, session.id);
        assert_eq!(note.content, "");
    }

    #[test]
    fn get_or_create_returns_existing() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = session_store::create(&conn, "Test").unwrap();
        let note1 = get_or_create(&conn, &session.id).unwrap();
        let note2 = get_or_create(&conn, &session.id).unwrap();

        assert_eq!(note1.id, note2.id);
    }

    #[test]
    fn update_content_persists() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = session_store::create(&conn, "Test").unwrap();
        let note = get_or_create(&conn, &session.id).unwrap();

        let updated = update_content(&conn, &note.id, "# Hello\n\nSome notes").unwrap();
        assert_eq!(updated.content, "# Hello\n\nSome notes");

        let fetched = get_or_create(&conn, &session.id).unwrap();
        assert_eq!(fetched.content, "# Hello\n\nSome notes");
    }

    #[test]
    fn note_deleted_with_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = session_store::create(&conn, "Test").unwrap();
        get_or_create(&conn, &session.id).unwrap();

        session_store::delete(&conn, &session.id).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
