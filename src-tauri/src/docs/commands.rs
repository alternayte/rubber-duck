use tauri::{AppHandle, State};

use crate::db::Database;

use super::model::{BuiltinTemplate, Document, DocumentSection, SectionVersion, Template};
use super::store;
use super::template_store;
use super::templates;

// ── built-in templates ────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_builtin_templates() -> Vec<BuiltinTemplate> {
    templates::get_builtin_templates()
}

// ── custom templates ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_custom_template(
    db: State<Database>,
    name: String,
    content: String,
) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::create_template(&conn, &name, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_custom_templates(db: State<Database>) -> Result<Vec<Template>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::list_templates(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_custom_template(db: State<Database>, template_id: String) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::get_template(&conn, &template_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_custom_template(
    db: State<Database>,
    template_id: String,
    name: String,
    content: String,
) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::update_template(&conn, &template_id, &name, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_custom_template(db: State<Database>, template_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::delete_template(&conn, &template_id).map_err(|e| e.to_string())
}

// ── documents ─────────────────────────────────────────────────────────────────

/// Create a document and all its sections in one command.
/// `sections` is a list of `[name, directive, sort_order]` triples serialized
/// from the parsed template on the frontend.
#[tauri::command]
pub fn create_doc(
    db: State<Database>,
    session_id: String,
    template_name: String,
    title: String,
    sections: Vec<serde_json::Value>,
) -> Result<(Document, Vec<DocumentSection>), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let doc = store::create_document(&conn, &session_id, &template_name, &title)
        .map_err(|e| e.to_string())?;

    let mut created_sections = Vec::new();
    for (i, section_val) in sections.iter().enumerate() {
        let name = section_val["name"]
            .as_str()
            .ok_or("section missing name")?
            .to_string();
        let directive = section_val["directive"]
            .as_str()
            .ok_or("section missing directive")?
            .to_string();
        let sort_order = section_val["sort_order"]
            .as_i64()
            .unwrap_or(i as i64) as i32;

        let section = store::create_section(&conn, &doc.id, &name, &directive, sort_order)
            .map_err(|e| e.to_string())?;
        created_sections.push(section);
    }

    Ok((doc, created_sections))
}

#[tauri::command]
pub fn get_doc(db: State<Database>, document_id: String) -> Result<Document, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::get_document(&conn, &document_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_docs(db: State<Database>, session_id: String) -> Result<Vec<Document>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_documents(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_doc(db: State<Database>, document_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::delete_document(&conn, &document_id).map_err(|e| e.to_string())
}

// ── sections ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_document_sections(
    db: State<Database>,
    document_id: String,
) -> Result<Vec<DocumentSection>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_sections(&conn, &document_id).map_err(|e| e.to_string())
}

/// Update section content manually (from the edit textarea).
/// Saves the current content as a version before overwriting.
#[tauri::command]
pub fn update_document_section(
    db: State<Database>,
    section_id: String,
    content: String,
) -> Result<DocumentSection, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let existing = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
    store::save_version(&conn, &section_id, &existing.content).map_err(|e| e.to_string())?;
    store::update_section_content(&conn, &section_id, &content).map_err(|e| e.to_string())
}

// ── generation ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_doc_section(
    app: AppHandle,
    db: State<'_, Database>,
    document_id: String,
    section_id: String,
) -> Result<(), String> {
    super::generator::generate_section(app, db, document_id, section_id).await
}

// ── versions ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_section_versions(
    db: State<Database>,
    section_id: String,
) -> Result<Vec<SectionVersion>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_versions(&conn, &section_id).map_err(|e| e.to_string())
}

/// Restore a section to a previous version.
/// Saves the current content as a new version first, then sets content to the restored version.
#[tauri::command]
pub fn restore_section_version(
    db: State<Database>,
    section_id: String,
    version_id: String,
) -> Result<DocumentSection, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let current = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
    store::save_version(&conn, &section_id, &current.content).map_err(|e| e.to_string())?;

    let version = store::get_version(&conn, &version_id).map_err(|e| e.to_string())?;
    store::update_section_content(&conn, &section_id, &version.content)
        .map_err(|e| e.to_string())
}
