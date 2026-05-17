use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::{Document, DocumentSection, SectionVersion};

// ── row mappers ────────────────────────────────────────────────────────────────

fn row_to_document(row: &rusqlite::Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        session_id: row.get(1)?,
        template_name: row.get(2)?,
        title: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn row_to_section(row: &rusqlite::Row) -> rusqlite::Result<DocumentSection> {
    Ok(DocumentSection {
        id: row.get(0)?,
        document_id: row.get(1)?,
        name: row.get(2)?,
        directive: row.get(3)?,
        content: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_version(row: &rusqlite::Row) -> rusqlite::Result<SectionVersion> {
    Ok(SectionVersion {
        id: row.get(0)?,
        section_id: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
    })
}

// ── documents ─────────────────────────────────────────────────────────────────

pub fn create_document(
    conn: &Connection,
    session_id: &str,
    template_name: &str,
    title: &str,
) -> AppResult<Document> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO documents (id, session_id, template_name, title)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, session_id, template_name, title],
    )?;
    get_document(conn, &id)
}

pub fn get_document(conn: &Connection, id: &str) -> AppResult<Document> {
    let doc = conn.query_row(
        "SELECT id, session_id, template_name, title, created_at, updated_at
         FROM documents WHERE id = ?1",
        params![id],
        row_to_document,
    )?;
    Ok(doc)
}

pub fn list_documents(conn: &Connection, session_id: &str) -> AppResult<Vec<Document>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, template_name, title, created_at, updated_at
         FROM documents WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let docs = stmt
        .query_map(params![session_id], row_to_document)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(docs)
}

pub fn delete_document(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
    Ok(())
}

// ── sections ──────────────────────────────────────────────────────────────────

pub fn create_section(
    conn: &Connection,
    document_id: &str,
    name: &str,
    directive: &str,
    sort_order: i32,
) -> AppResult<DocumentSection> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO document_sections (id, document_id, name, directive, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, document_id, name, directive, sort_order],
    )?;
    get_section(conn, &id)
}

pub fn get_section(conn: &Connection, id: &str) -> AppResult<DocumentSection> {
    let section = conn.query_row(
        "SELECT id, document_id, name, directive, content, sort_order, created_at, updated_at
         FROM document_sections WHERE id = ?1",
        params![id],
        row_to_section,
    )?;
    Ok(section)
}

pub fn list_sections(conn: &Connection, document_id: &str) -> AppResult<Vec<DocumentSection>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, name, directive, content, sort_order, created_at, updated_at
         FROM document_sections WHERE document_id = ?1 ORDER BY sort_order ASC",
    )?;
    let sections = stmt
        .query_map(params![document_id], row_to_section)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sections)
}

pub fn update_section_content(
    conn: &Connection,
    section_id: &str,
    content: &str,
) -> AppResult<DocumentSection> {
    conn.execute(
        "UPDATE document_sections SET content = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![content, section_id],
    )?;
    get_section(conn, section_id)
}

// ── versions ──────────────────────────────────────────────────────────────────

/// Save the current content of a section as a version snapshot.
/// Empty content is not saved (initial state before first generation).
pub fn save_version(conn: &Connection, section_id: &str, content: &str) -> AppResult<()> {
    if content.is_empty() {
        return Ok(());
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO document_section_versions (id, section_id, content) VALUES (?1, ?2, ?3)",
        params![id, section_id, content],
    )?;
    Ok(())
}

pub fn list_versions(conn: &Connection, section_id: &str) -> AppResult<Vec<SectionVersion>> {
    let mut stmt = conn.prepare(
        "SELECT id, section_id, content, created_at
         FROM document_section_versions WHERE section_id = ?1
         ORDER BY created_at DESC, rowid DESC",
    )?;
    let versions = stmt
        .query_map(params![section_id], row_to_version)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(versions)
}

pub fn get_version(conn: &Connection, version_id: &str) -> AppResult<SectionVersion> {
    let version = conn.query_row(
        "SELECT id, section_id, content, created_at FROM document_section_versions WHERE id = ?1",
        params![version_id],
        row_to_version,
    )?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::session::store as session_store;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn make_session(conn: &Connection) -> String {
        session_store::create(conn, "Test Session").unwrap().id
    }

    fn make_doc(conn: &Connection, session_id: &str) -> Document {
        create_document(conn, session_id, "PRD", "My PRD").unwrap()
    }

    #[test]
    fn create_and_get_document() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let doc = create_document(&conn, &session_id, "PRD", "Auth Flow PRD").unwrap();
        assert_eq!(doc.session_id, session_id);
        assert_eq!(doc.template_name, "PRD");
        assert_eq!(doc.title, "Auth Flow PRD");

        let fetched = get_document(&conn, &doc.id).unwrap();
        assert_eq!(fetched.id, doc.id);
    }

    #[test]
    fn list_documents_ordered_by_created_at() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        create_document(&conn, &session_id, "PRD", "First").unwrap();
        create_document(&conn, &session_id, "SDD", "Second").unwrap();

        let docs = list_documents(&conn, &session_id).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "First");
        assert_eq!(docs[1].title, "Second");
    }

    #[test]
    fn delete_document_cascades_sections() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);

        create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();
        delete_document(&conn, &doc.id).unwrap();

        let sections = list_sections(&conn, &doc.id).unwrap();
        assert!(sections.is_empty());
    }

    #[test]
    fn create_and_list_sections() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);

        create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();
        create_section(&conn, &doc.id, "Requirements", "List requirements", 1).unwrap();

        let sections = list_sections(&conn, &doc.id).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[0].sort_order, 0);
        assert_eq!(sections[0].content, "");
        assert_eq!(sections[1].name, "Requirements");
    }

    #[test]
    fn update_section_content_changes_content() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        let updated = update_section_content(&conn, &section.id, "This is the overview.").unwrap();
        assert_eq!(updated.content, "This is the overview.");
    }

    #[test]
    fn save_version_skips_empty_content() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "").unwrap();
        let versions = list_versions(&conn, &section.id).unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn save_and_list_versions() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "Version 1 content").unwrap();
        save_version(&conn, &section.id, "Version 2 content").unwrap();

        let versions = list_versions(&conn, &section.id).unwrap();
        assert_eq!(versions.len(), 2);
        // Ordered newest first
        assert_eq!(versions[0].content, "Version 2 content");
        assert_eq!(versions[1].content, "Version 1 content");
    }

    #[test]
    fn get_version_by_id() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "Saved content").unwrap();
        let versions = list_versions(&conn, &section.id).unwrap();

        let fetched = get_version(&conn, &versions[0].id).unwrap();
        assert_eq!(fetched.content, "Saved content");
        assert_eq!(fetched.section_id, section.id);
    }

    #[test]
    fn cascade_delete_with_session() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write", 0).unwrap();
        save_version(&conn, &section.id, "Some content").unwrap();

        crate::session::store::delete(&conn, &session_id).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
