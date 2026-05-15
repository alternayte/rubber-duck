use tauri::State;

use crate::db::Database;

use super::model::{CreateTicketParams, Ticket, UpdateTicketParams};
use super::store;

#[tauri::command]
pub fn create_ticket(db: State<Database>, params: CreateTicketParams) -> Result<Ticket, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::create(&conn, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ticket(db: State<Database>, id: String) -> Result<Ticket, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::get(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_tickets(db: State<Database>, session_id: String) -> Result<Vec<Ticket>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_by_session(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_ticket(
    db: State<Database>,
    id: String,
    params: UpdateTicketParams,
) -> Result<Ticket, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::update(&conn, &id, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_ticket(db: State<Database>, id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::delete(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_ticket(db: State<Database>, id: String, sort_order: i32) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::reorder(&conn, &id, sort_order).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_ticket_parent(
    db: State<Database>,
    id: String,
    parent_id: Option<String>,
) -> Result<Ticket, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::set_parent(&conn, &id, parent_id.as_deref()).map_err(|e| e.to_string())
}
