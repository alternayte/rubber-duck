use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::db::Database;
use crate::jira::client::{extract_jira_keys, JiraClient};
use crate::jira::model::JiraIssueContext;
use crate::session::conversation_store;
use crate::session::store as session_store;
use crate::session::note_store;
use crate::settings::commands::get_api_key_from_keyring;
use crate::settings::store as settings_store;

use super::client::{self, StreamEvent};
use super::context;

#[derive(Clone, Serialize)]
struct ChunkPayload {
    content: String,
}

#[derive(Clone, Serialize)]
struct DonePayload {
    full_content: String,
}

#[derive(Clone, Serialize)]
struct ErrorPayload {
    message: String,
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    db: State<'_, Database>,
    session_id: String,
    content: String,
    mode: String,
) -> Result<(), String> {
    let api_key = get_api_key_from_keyring().map_err(|e| e.to_string())?;

    let chat_mode = if mode == "grill" {
        context::ChatMode::Grill
    } else {
        context::ChatMode::Assist
    };

    let (session_context, note_content, tickets, conversation, jira_keys) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        conversation_store::save_message(&conn, &session_id, "User", &content)
            .map_err(|e| e.to_string())?;

        let session = session_store::get(&conn, &session_id).map_err(|e| e.to_string())?;

        let note_content = note_store::get_or_create(&conn, &session_id)
            .map(|n| n.content)
            .unwrap_or_default();

        let mut ticket_stmt = conn
            .prepare(
                "SELECT title, ticket_type, priority, description FROM tickets WHERE session_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let tickets: Vec<(String, String, String, String)> = ticket_stmt
            .query_map(params![session_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut conv_stmt = conn
            .prepare(
                "SELECT role, content FROM conversations WHERE session_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let conversation: Vec<(String, String)> = conv_stmt
            .query_map(params![session_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut all_text = note_content.clone();
        all_text.push(' ');
        all_text.push_str(&content);
        let jira_keys = extract_jira_keys(&all_text);

        (session.context, note_content, tickets, conversation, jira_keys)
    };

    let jira_issues: Vec<JiraIssueContext> = if !jira_keys.is_empty() {
        match crate::jira::commands::get_jira_credentials(&db) {
            Ok((base_url, auth)) => match JiraClient::new(&base_url, auth) {
                Ok(client) => {
                    let mut issues = Vec::new();
                    for key in &jira_keys {
                        match client.get_issue(key).await {
                            Ok(issue) => issues.push(issue),
                            Err(e) => tracing::warn!("Failed to fetch Jira issue {key}: {e}"),
                        }
                    }
                    issues
                }
                Err(_) => vec![],
            },
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    let (messages, model) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        let messages = context::assemble_context(
            &chat_mode,
            &session_context,
            &note_content,
            &tickets,
            &jira_issues,
            &conversation,
        );

        let model = settings_store::get(&conn, "llm.model")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| super::models::DEFAULT_MODEL.to_string());

        (messages, model)
    };

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);

    let app_clone = app.clone();
    let db_clone_session_id = session_id.clone();

    tokio::spawn(async move {
        client::stream_completion(&api_key, &model, messages, tx).await;
    });

    tokio::spawn(async move {
        let mut full_content = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    let _ = app_clone.emit("llm:chunk", ChunkPayload { content: text });
                }
                StreamEvent::Done(text) => {
                    full_content = text;
                    let _ = app_clone.emit(
                        "llm:done",
                        DonePayload {
                            full_content: full_content.clone(),
                        },
                    );
                    break;
                }
                StreamEvent::Error(msg) => {
                    let _ = app_clone.emit("llm:error", ErrorPayload { message: msg });
                    return;
                }
            }
        }

        if !full_content.is_empty() {
            let db: State<Database> = app_clone.state::<Database>();
            if let Ok(conn) = db.conn() {
                if let Err(e) = conversation_store::save_message(
                    &conn,
                    &db_clone_session_id,
                    "Assistant",
                    &full_content,
                ) {
                    tracing::error!("Failed to save assistant message: {e}");
                    let _ = app_clone.emit(
                        "llm:error",
                        ErrorPayload {
                            message: "Message displayed but failed to save — try sending again"
                                .to_string(),
                        },
                    );
                }
            };
        }
    });

    Ok(())
}
