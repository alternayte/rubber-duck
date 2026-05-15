use rusqlite::{params, Connection};

use crate::error::AppResult;

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str, category: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value, category, updated_at)
         VALUES (?1, ?2, ?3, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, category],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_by_category(conn: &Connection, category: &str) -> AppResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM settings WHERE category = ?1 ORDER BY key",
    )?;
    let rows = stmt
        .query_map(params![category], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn get_returns_default_setting() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let value = get(&conn, "llm.model").unwrap();
        assert_eq!(value, Some("deepseek/deepseek-chat-v4-0324:free".to_string()));
    }

    #[test]
    fn get_returns_none_for_missing_key() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let value = get(&conn, "nonexistent.key").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn set_updates_existing_key() {
        let db = test_db();
        let conn = db.conn().unwrap();

        set(&conn, "llm.model", "openai/gpt-4o", "llm").unwrap();
        let value = get(&conn, "llm.model").unwrap();
        assert_eq!(value, Some("openai/gpt-4o".to_string()));
    }

    #[test]
    fn set_creates_new_key() {
        let db = test_db();
        let conn = db.conn().unwrap();

        set(&conn, "llm.temperature", "0.7", "llm").unwrap();
        let value = get(&conn, "llm.temperature").unwrap();
        assert_eq!(value, Some("0.7".to_string()));
    }

    #[test]
    fn get_by_category_returns_matching_entries() {
        let db = test_db();
        let conn = db.conn().unwrap();

        set(&conn, "ui.theme", "dark", "ui").unwrap();
        set(&conn, "ui.font_size", "14", "ui").unwrap();

        let llm_entries = get_by_category(&conn, "llm").unwrap();
        assert_eq!(llm_entries.len(), 2);

        let keys: Vec<&str> = llm_entries.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"llm.api_key_ref"));
        assert!(keys.contains(&"llm.model"));

        let ui_entries = get_by_category(&conn, "ui").unwrap();
        assert_eq!(ui_entries.len(), 2);

        let ui_keys: Vec<&str> = ui_entries.iter().map(|(k, _)| k.as_str()).collect();
        assert!(ui_keys.contains(&"ui.font_size"));
        assert!(ui_keys.contains(&"ui.theme"));
    }
}
