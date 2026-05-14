use tauri::State;

use crate::db::Database;
use crate::error::AppError;

use super::store;

const KEYRING_SERVICE: &str = "rubber-duck";
const KEYRING_USER: &str = "openrouter-api-key";

#[tauri::command]
pub fn get_setting(db: State<Database>, key: String) -> Result<Option<String>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::get(&conn, &key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(
    db: State<Database>,
    key: String,
    value: String,
    category: String,
) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::set(&conn, &key, &value, &category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_api_key(db: State<Database>, key: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;
    entry
        .set_password(&key)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;

    let conn = db.conn().map_err(|e| e.to_string())?;
    store::set(&conn, "llm.api_key_ref", "openrouter", "llm").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn has_api_key(db: State<Database>) -> Result<bool, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let ref_value = store::get(&conn, "llm.api_key_ref").map_err(|e| e.to_string())?;
    Ok(ref_value.is_some_and(|v| !v.is_empty()))
}

#[tauri::command]
pub fn get_available_models() -> Vec<crate::llm::models::ModelInfo> {
    crate::llm::models::MODELS.to_vec()
}

pub fn get_api_key_from_keyring() -> Result<String, AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()))?;
    entry
        .get_password()
        .map_err(|e| AppError::Keyring(e.to_string()))
}
