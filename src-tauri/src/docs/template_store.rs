use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::Template;

fn row_to_template(row: &rusqlite::Row) -> rusqlite::Result<Template> {
    Ok(Template {
        id: row.get(0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

pub fn create_template(conn: &Connection, name: &str, content: &str) -> AppResult<Template> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO templates (id, name, content) VALUES (?1, ?2, ?3)",
        params![id, name, content],
    )?;
    get_template(conn, &id)
}

pub fn get_template(conn: &Connection, id: &str) -> AppResult<Template> {
    let template = conn.query_row(
        "SELECT id, name, content, created_at, updated_at FROM templates WHERE id = ?1",
        params![id],
        row_to_template,
    )?;
    Ok(template)
}

pub fn list_templates(conn: &Connection) -> AppResult<Vec<Template>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, content, created_at, updated_at FROM templates ORDER BY created_at ASC",
    )?;
    let templates = stmt
        .query_map([], row_to_template)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(templates)
}

pub fn update_template(
    conn: &Connection,
    id: &str,
    name: &str,
    content: &str,
) -> AppResult<Template> {
    conn.execute(
        "UPDATE templates SET name = ?1, content = ?2, updated_at = datetime('now') WHERE id = ?3",
        params![name, content, id],
    )?;
    get_template(conn, id)
}

pub fn delete_template(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM templates WHERE id = ?1", params![id])?;
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
    fn create_and_get_template() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "My Template", "<!-- section: Overview -->\n<!-- directive: Write overview. -->").unwrap();
        assert_eq!(tmpl.name, "My Template");
        assert!(!tmpl.id.is_empty());

        let fetched = get_template(&conn, &tmpl.id).unwrap();
        assert_eq!(fetched.id, tmpl.id);
        assert_eq!(fetched.name, "My Template");
    }

    #[test]
    fn list_templates_ordered_by_created_at() {
        let db = test_db();
        let conn = db.conn().unwrap();

        create_template(&conn, "Alpha", "content").unwrap();
        create_template(&conn, "Beta", "content").unwrap();

        let templates = list_templates(&conn).unwrap();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "Alpha");
        assert_eq!(templates[1].name, "Beta");
    }

    #[test]
    fn update_template_changes_name_and_content() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "Old Name", "old content").unwrap();
        let updated = update_template(&conn, &tmpl.id, "New Name", "new content").unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.content, "new content");
    }

    #[test]
    fn delete_template_removes_it() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "To Delete", "content").unwrap();
        delete_template(&conn, &tmpl.id).unwrap();

        let result = get_template(&conn, &tmpl.id);
        assert!(result.is_err());
    }

    #[test]
    fn list_empty_returns_empty_vec() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let templates = list_templates(&conn).unwrap();
        assert!(templates.is_empty());
    }
}
