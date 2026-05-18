use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub content_type: String,
    pub session_id: String,
    pub session_name: String,
    pub thread_id: Option<String>,
    pub source_id: String,
    pub preview: String,
}

pub fn search_all(conn: &Connection, query: &str) -> AppResult<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();

    // Search conversations via FTS5
    let fts_query = format!("\"{}\"", query.replace('"', "\"\""));
    if let Ok(mut stmt) = conn.prepare(
        "SELECT snippet(conversations_fts, 0, '**', '**', '...', 40) as preview,
                cf.conversation_id, cf.session_id, s.title as session_name,
                c.thread_id
         FROM conversations_fts cf
         JOIN conversations c ON c.id = cf.conversation_id
         JOIN sessions s ON s.id = cf.session_id
         WHERE conversations_fts MATCH ?1
         ORDER BY rank
         LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map(params![fts_query], |row| {
            Ok(SearchResult {
                content_type: "chat".to_string(),
                preview: row.get(0)?,
                source_id: row.get(1)?,
                session_id: row.get(2)?,
                session_name: row.get(3)?,
                thread_id: row.get(4)?,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    // Search notes via LIKE (no FTS on notes)
    if let Ok(mut stmt) = conn.prepare(
        "SELECT n.content, n.session_id, s.title as session_name, n.id
         FROM notes n
         JOIN sessions s ON s.id = n.session_id
         WHERE n.content LIKE ?1
         LIMIT 10"
    ) {
        let like_query = format!("%{query}%");
        if let Ok(rows) = stmt.query_map(params![like_query], |row| {
            let content: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let session_name: String = row.get(2)?;
            let source_id: String = row.get(3)?;

            let lower = content.to_lowercase();
            let q_lower = query.to_lowercase();
            let preview = if let Some(pos) = lower.find(&q_lower) {
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(content.len());
                let slice = &content[start..end];
                if start > 0 { format!("...{slice}...") } else { format!("{slice}...") }
            } else {
                content.chars().take(80).collect::<String>()
            };

            Ok(SearchResult {
                content_type: "note".to_string(),
                preview,
                source_id,
                session_id,
                session_name,
                thread_id: None,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    // Search document sections via LIKE
    if let Ok(mut stmt) = conn.prepare(
        "SELECT ds.content, d.session_id, s.title as session_name, ds.id, d.title
         FROM document_sections ds
         JOIN documents d ON d.id = ds.document_id
         JOIN sessions s ON s.id = d.session_id
         WHERE ds.content LIKE ?1
         LIMIT 10"
    ) {
        let like_query = format!("%{query}%");
        if let Ok(rows) = stmt.query_map(params![like_query], |row| {
            let content: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let session_name: String = row.get(2)?;
            let source_id: String = row.get(3)?;

            let lower = content.to_lowercase();
            let q_lower = query.to_lowercase();
            let preview = if let Some(pos) = lower.find(&q_lower) {
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(content.len());
                let slice = &content[start..end];
                if start > 0 { format!("...{slice}...") } else { format!("{slice}...") }
            } else {
                content.chars().take(80).collect::<String>()
            };

            Ok(SearchResult {
                content_type: "doc".to_string(),
                preview,
                source_id,
                session_id,
                session_name,
                thread_id: None,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    Ok(results)
}
