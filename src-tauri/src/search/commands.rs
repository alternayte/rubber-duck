use tauri::State;

use crate::db::Database;
use super::store::{self, SearchResult};

#[tauri::command]
pub fn search_all(db: State<Database>, query: String) -> Result<Vec<SearchResult>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::search_all(&conn, &query).map_err(|e| e.to_string())
}
