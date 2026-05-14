use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::{Session, SessionStatus};

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<Session> {
    let status_str: String = row.get(3)?;
    Ok(Session {
        id: row.get(0)?,
        title: row.get(1)?,
        context: row.get(2)?,
        status: SessionStatus::parse(&status_str),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub fn create(conn: &Connection, title: &str) -> AppResult<Session> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sessions (id, title, status) VALUES (?1, ?2, 'Active')",
        params![id, title],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Session> {
    let session = conn.query_row(
        "SELECT id, title, context, status, created_at, updated_at
         FROM sessions WHERE id = ?1",
        params![id],
        row_to_session,
    )?;
    Ok(session)
}

pub fn list(conn: &Connection) -> AppResult<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, context, status, created_at, updated_at
         FROM sessions WHERE status != 'Archived'
         ORDER BY updated_at DESC",
    )?;
    let sessions = stmt
        .query_map([], row_to_session)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

pub fn update(
    conn: &Connection,
    id: &str,
    title: &str,
    context: &str,
) -> AppResult<Session> {
    conn.execute(
        "UPDATE sessions SET title = ?1, context = ?2, updated_at = datetime('now')
         WHERE id = ?3",
        params![title, context, id],
    )?;
    get(conn, id)
}

pub fn archive(conn: &Connection, id: &str) -> AppResult<Session> {
    conn.execute(
        "UPDATE sessions SET status = 'Archived', updated_at = datetime('now')
         WHERE id = ?1",
        params![id],
    )?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = create(&conn, "Test Session").unwrap();
        assert_eq!(session.title, "Test Session");
        assert_eq!(session.status, SessionStatus::Active);
        assert_eq!(session.context, "");

        let fetched = get(&conn, &session.id).unwrap();
        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.title, "Test Session");
    }

    #[test]
    fn list_excludes_archived() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let s1 = create(&conn, "Session 1").unwrap();
        let _s2 = create(&conn, "Session 2").unwrap();
        archive(&conn, &s1.id).unwrap();

        let sessions = list(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title, "Session 2");
    }

    #[test]
    fn list_ordered_by_updated_at_desc() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let s1 = create(&conn, "First").unwrap();
        let _s2 = create(&conn, "Second").unwrap();

        update(&conn, &s1.id, "First Updated", "").unwrap();

        let sessions = list(&conn).unwrap();
        assert_eq!(sessions[0].title, "First Updated");
    }

    #[test]
    fn update_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = create(&conn, "Original").unwrap();
        let updated = update(&conn, &session.id, "Renamed", "Some context").unwrap();

        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.context, "Some context");
    }

    #[test]
    fn archive_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = create(&conn, "To Archive").unwrap();
        let archived = archive(&conn, &session.id).unwrap();

        assert_eq!(archived.status, SessionStatus::Archived);
    }

    #[test]
    fn delete_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = create(&conn, "To Delete").unwrap();
        delete(&conn, &session.id).unwrap();

        let result = get(&conn, &session.id);
        assert!(result.is_err());
    }

    #[test]
    fn delete_cascades_to_children() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let session = create(&conn, "Parent").unwrap();
        conn.execute(
            "INSERT INTO notes (id, session_id, content) VALUES (?1, ?2, ?3)",
            params!["n1", session.id, "note content"],
        )
        .unwrap();

        delete(&conn, &session.id).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
