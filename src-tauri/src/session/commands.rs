use tauri::Manager;
use tauri::State;

use crate::db::Database;

use super::model::{ConversationMessage, Note, Session};
use super::note_store;
use super::store;

#[tauri::command]
pub fn create_session(db: State<Database>, title: String) -> Result<Session, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::create(&conn, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session(db: State<Database>, id: String) -> Result<Session, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::get(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sessions(db: State<Database>) -> Result<Vec<Session>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_session(
    db: State<Database>,
    id: String,
    title: String,
    context: String,
) -> Result<Session, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::update(&conn, &id, &title, &context).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn archive_session(db: State<Database>, id: String) -> Result<Session, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::archive(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_or_create_note(
    db: State<Database>,
    session_id: String,
) -> Result<Note, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    note_store::get_or_create(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_note(
    db: State<Database>,
    id: String,
    content: String,
) -> Result<Note, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    note_store::update_content(&conn, &id, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_conversation(
    db: State<Database>,
    session_id: String,
) -> Result<Vec<ConversationMessage>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, created_at FROM conversations
             WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let messages = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(messages)
}

#[tauri::command]
pub fn save_pasted_image(
    app: tauri::AppHandle,
    session_id: String,
    base64_data: String,
) -> Result<String, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let dir_str = app_data_dir.to_string_lossy().to_string();
    super::image_store::save_image(&dir_str, &session_id, &base64_data)
        .map_err(|e| e.to_string())
}
