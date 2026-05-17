use std::sync::{Mutex, Once};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

static SQLITE_VEC_INIT: Once = Once::new();

fn ensure_sqlite_vec_loaded() {
    SQLITE_VEC_INIT.call_once(|| unsafe {
        let rc = rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
        assert_eq!(rc, rusqlite::ffi::SQLITE_OK);
    });
}

struct Migration {
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "001_initial_schema",
        sql: include_str!("../migrations/001_initial_schema.sql"),
    },
    Migration {
        name: "002_add_fts",
        sql: include_str!("../migrations/002_add_fts.sql"),
    },
    Migration {
        name: "003_add_settings",
        sql: include_str!("../migrations/003_add_settings.sql"),
    },
    Migration {
        name: "004_add_repos",
        sql: include_str!("../migrations/004_add_repos.sql"),
    },
    Migration {
        name: "005_add_docs",
        sql: include_str!("../migrations/005_add_docs.sql"),
    },
    Migration {
        name: "006_add_rag",
        sql: include_str!("../migrations/006_add_rag.sql"),
    },
];

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> AppResult<Self> {
        ensure_sqlite_vec_loaded();
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        ensure_sqlite_vec_loaded();
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> AppResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> AppResult<()> {
        let conn = self.conn.lock().map_err(|e| AppError::Other(e.to_string()))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )?;

        for migration in MIGRATIONS {
            let applied: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM _migrations WHERE name = ?1",
                [migration.name],
                |row| row.get(0),
            )?;

            if !applied {
                conn.execute_batch(migration.sql)?;
                conn.execute(
                    "INSERT INTO _migrations (name) VALUES (?1)",
                    [migration.name],
                )?;
            }
        }

        Ok(())
    }

    pub fn conn(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| AppError::Other(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_on_fresh_db() {
        let db = Database::open_in_memory().expect("failed to create db");
        let conn = db.conn().unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn migrations_are_idempotent() {
        let db = Database::open_in_memory().expect("failed to create db");
        drop(db);

        let db2 = Database::open_in_memory().expect("failed to create db on second init");
        let conn = db2.conn().unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn sessions_table_exists() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["test-id", "Test Session"],
        )
        .unwrap();

        let title: String = conn
            .query_row(
                "SELECT title FROM sessions WHERE id = ?1",
                ["test-id"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Test Session");
    }

    #[test]
    fn notes_table_exists_with_fk() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["s1", "Session"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO notes (id, session_id, content) VALUES (?1, ?2, ?3)",
            ["n1", "s1", "Some notes"],
        )
        .unwrap();

        let content: String = conn
            .query_row("SELECT content FROM notes WHERE id = ?1", ["n1"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(content, "Some notes");
    }

    #[test]
    fn tickets_table_exists() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["s1", "Session"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tickets (id, session_id, title) VALUES (?1, ?2, ?3)",
            ["t1", "s1", "Fix the bug"],
        )
        .unwrap();

        let title: String = conn
            .query_row(
                "SELECT title FROM tickets WHERE id = ?1",
                ["t1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Fix the bug");
    }

    #[test]
    fn conversations_table_exists() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["s1", "Session"],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, ?3, ?4)",
            ["c1", "s1", "User", "Hello duck"],
        )
        .unwrap();

        let role: String = conn
            .query_row(
                "SELECT role FROM conversations WHERE id = ?1",
                ["c1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(role, "User");
    }

    #[test]
    fn fts5_search_index_works() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO search_index (title, body, source_type, source_id, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            ["Bug fix", "Fix the login bug in auth module", "ticket", "t1", "s1"],
        )
        .unwrap();

        let found: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM search_index WHERE search_index MATCH ?1",
                ["login"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(found);
    }

    #[test]
    fn cascade_deletes_work() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["s1", "Session"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes (id, session_id, content) VALUES (?1, ?2, ?3)",
            ["n1", "s1", "note"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tickets (id, session_id, title) VALUES (?1, ?2, ?3)",
            ["t1", "s1", "ticket"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, ?3, ?4)",
            ["c1", "s1", "User", "msg"],
        )
        .unwrap();

        conn.execute("DELETE FROM sessions WHERE id = ?1", ["s1"])
            .unwrap();

        let notes: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
            .unwrap();
        let tickets: i64 = conn
            .query_row("SELECT COUNT(*) FROM tickets", [], |row| row.get(0))
            .unwrap();
        let convos: i64 = conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(notes, 0);
        assert_eq!(tickets, 0);
        assert_eq!(convos, 0);
    }

    #[test]
    fn default_values_applied() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            ["s1", "Session"],
        )
        .unwrap();

        let (status, context): (String, String) = conn
            .query_row(
                "SELECT status, context FROM sessions WHERE id = ?1",
                ["s1"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(status, "Draft");
        assert_eq!(context, "");

        conn.execute(
            "INSERT INTO tickets (id, session_id, title) VALUES (?1, ?2, ?3)",
            ["t1", "s1", "Ticket"],
        )
        .unwrap();

        let (priority, ticket_type, labels, status): (String, String, String, String) = conn
            .query_row(
                "SELECT priority, ticket_type, labels, status FROM tickets WHERE id = ?1",
                ["t1"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(priority, "Medium");
        assert_eq!(ticket_type, "Task");
        assert_eq!(labels, "[]");
        assert_eq!(status, "Draft");
    }
}
