use rusqlite::{params, Connection};

use crate::error::AppResult;
use super::model::RepoContext;

fn row_to_repo(row: &rusqlite::Row) -> rusqlite::Result<RepoContext> {
    Ok(RepoContext {
        id: row.get(0)?,
        session_id: row.get(1)?,
        name: row.get(2)?,
        source: row.get(3)?,
        local_path: row.get(4)?,
        created_at: row.get(5)?,
    })
}

pub fn attach(
    conn: &Connection,
    session_id: &str,
    name: &str,
    source: &str,
    local_path: &str,
) -> AppResult<RepoContext> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO repos (id, session_id, name, source, local_path) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_id, name, source, local_path],
    )?;
    get(conn, &id)
}

pub fn detach(conn: &Connection, repo_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM repos WHERE id = ?1", params![repo_id])?;
    Ok(())
}

pub fn list_by_session(conn: &Connection, session_id: &str) -> AppResult<Vec<RepoContext>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, name, source, local_path, created_at FROM repos WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let repos = stmt.query_map(params![session_id], row_to_repo)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(repos)
}

pub fn get(conn: &Connection, repo_id: &str) -> AppResult<RepoContext> {
    let repo = conn.query_row(
        "SELECT id, session_id, name, source, local_path, created_at FROM repos WHERE id = ?1",
        params![repo_id],
        row_to_repo,
    )?;
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn attach_and_get() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        let repo = attach(&conn, "s1", "my-repo", "/local/path", "/local/path").unwrap();
        assert_eq!(repo.name, "my-repo");
        assert_eq!(repo.source, "/local/path");
        assert_eq!(repo.local_path, "/local/path");

        let fetched = get(&conn, &repo.id).unwrap();
        assert_eq!(fetched.name, "my-repo");
    }

    #[test]
    fn list_by_session_returns_attached_repos() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s2", "Other"]).unwrap();

        attach(&conn, "s1", "repo-a", "/a", "/a").unwrap();
        attach(&conn, "s1", "repo-b", "/b", "/b").unwrap();
        attach(&conn, "s2", "repo-c", "/c", "/c").unwrap();

        let repos = list_by_session(&conn, "s1").unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "repo-a");
        assert_eq!(repos[1].name, "repo-b");
    }

    #[test]
    fn detach_removes_repo() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        let repo = attach(&conn, "s1", "my-repo", "/path", "/path").unwrap();
        detach(&conn, &repo.id).unwrap();

        let repos = list_by_session(&conn, "s1").unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn cascade_delete_on_session() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        attach(&conn, "s1", "my-repo", "/path", "/path").unwrap();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params!["s1"]).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
    }
}
