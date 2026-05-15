use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Database;
use crate::error::AppError;
use crate::settings::store as settings_store;

use super::client::JiraClient;
use super::model::{JiraAuth, JiraUser};

const KEYRING_SERVICE: &str = "rubber-duck";
const JIRA_KEYRING_USER: &str = "jira-api-token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    pub base_url: String,
    pub auth_method: String,   // "basic" or "pat"
    pub email: Option<String>, // Only needed for basic auth
}

fn get_jira_credentials(db: &Database) -> Result<(String, JiraAuth), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let base_url = settings_store::get(&conn, "jira.base_url")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Jira base URL not configured".to_string())?;
    let auth_method = settings_store::get(&conn, "jira.auth_method")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "basic".to_string());

    let entry = keyring::Entry::new(KEYRING_SERVICE, JIRA_KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;
    let token = entry
        .get_password()
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;

    let auth = if auth_method == "pat" {
        JiraAuth::Pat(token)
    } else {
        let email = settings_store::get(&conn, "jira.email")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Jira email not configured".to_string())?;
        JiraAuth::Basic { email, api_token: token }
    };

    Ok((base_url, auth))
}

#[tauri::command]
pub fn get_jira_config(db: State<Database>) -> Result<Option<JiraConfig>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let base_url = settings_store::get(&conn, "jira.base_url").map_err(|e| e.to_string())?;
    let auth_method = settings_store::get(&conn, "jira.auth_method").map_err(|e| e.to_string())?;

    match base_url {
        Some(base_url) => {
            let email = settings_store::get(&conn, "jira.email").map_err(|e| e.to_string())?;
            Ok(Some(JiraConfig {
                base_url,
                auth_method: auth_method.unwrap_or_else(|| "basic".to_string()),
                email,
            }))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn set_jira_config(
    db: State<Database>,
    base_url: String,
    auth_method: String,
    email: Option<String>,
) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let normalized = base_url.trim_end_matches('/').to_string();
    settings_store::set(&conn, "jira.base_url", &normalized, "jira").map_err(|e| e.to_string())?;
    settings_store::set(&conn, "jira.auth_method", &auth_method, "jira").map_err(|e| e.to_string())?;
    if let Some(email) = &email {
        settings_store::set(&conn, "jira.email", email, "jira").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_jira_api_token(key: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, JIRA_KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;
    entry
        .set_password(&key)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;
    Ok(())
}

#[tauri::command]
pub fn has_jira_config(db: State<Database>) -> Result<bool, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let has_url = settings_store::get(&conn, "jira.base_url")
        .map_err(|e| e.to_string())?
        .is_some_and(|v| !v.is_empty());
    let auth_method = settings_store::get(&conn, "jira.auth_method")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "basic".to_string());
    let has_token = keyring::Entry::new(KEYRING_SERVICE, JIRA_KEYRING_USER)
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some_and(|v| !v.is_empty());

    if auth_method == "pat" {
        Ok(has_url && has_token)
    } else {
        let has_email = settings_store::get(&conn, "jira.email")
            .map_err(|e| e.to_string())?
            .is_some_and(|v| !v.is_empty());
        Ok(has_url && has_email && has_token)
    }
}

#[tauri::command]
pub async fn test_jira_connection(db: State<'_, Database>) -> Result<JiraUser, String> {
    let (base_url, auth) = get_jira_credentials(&db)?;
    let client = JiraClient::new(&base_url, auth).map_err(|e| e.to_string())?;
    client.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn push_ticket_to_jira(
    db: State<'_, Database>,
    ticket_id: String,
    project_key: String,
) -> Result<crate::ticket::model::Ticket, String> {
    let (base_url, auth) = get_jira_credentials(&db)?;
    let client = JiraClient::new(&base_url, auth).map_err(|e| e.to_string())?;

    let ticket = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        crate::ticket::store::get(&conn, &ticket_id).map_err(|e| e.to_string())?
    };

    let ext_ref = client
        .create_issue(&project_key, &ticket.title, &ticket.description, &ticket.ticket_type)
        .await
        .map_err(|e| e.to_string())?;

    let ext_ref_json = serde_json::to_string(&ext_ref).map_err(|e| e.to_string())?;

    let conn = db.conn().map_err(|e| e.to_string())?;
    crate::ticket::store::set_external_ref(&conn, &ticket_id, Some(&ext_ref_json))
        .map_err(|e| e.to_string())
}
