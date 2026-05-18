use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::db::Database;
use crate::CancellationTokens;
use crate::jira::client::{extract_jira_keys, JiraClient};
use crate::jira::model::JiraIssueContext;
use crate::llm::client::{self, StreamEvent};
use crate::llm::context::ChatMessage;
use crate::llm::models;
use crate::rag::{embedder::Embedder, model::RetrievedChunk, search as rag_search};
use crate::repo_context::{model::RepoFileContext, store as repo_store, tree as repo_tree};
use crate::session::note_store;
use crate::settings::commands::get_api_key_from_keyring;
use crate::settings::store as settings_store;

use super::store;

#[derive(Clone, Serialize)]
struct DocChunkPayload {
    section_id: String,
    content: String,
}

#[derive(Clone, Serialize)]
struct DocDonePayload {
    section_id: String,
    full_content: String,
}

#[derive(Clone, Serialize)]
struct DocErrorPayload {
    section_id: String,
    message: String,
}

const DOC_SYSTEM_PROMPT: &str = "You are generating a section of a technical document \
for a team that uses agentic development practices (AI coding agents implement tickets, \
automated verification is the norm, human review focuses on architecture and business logic). \
\n\nWrite in clear, professional prose. Use markdown formatting (headings, lists, code blocks). \
Be specific and grounded in the session context — do not pad with generic boilerplate. \
\n\nKey principles for this era:\n\
- Specifications should be precise enough for an AI agent to implement without ambiguity.\n\
- Distinguish automated verification (tests, linting, type checks) from human judgment (UX, business logic, architecture).\n\
- Don't prescribe manual processes for things agents can automate (code review, test generation, security scanning).\n\
- Focus on WHAT and WHY. The HOW is increasingly delegated to agents.\n\
\nWrite only the content for this section; do not include the section heading or any preamble.";

pub async fn generate_section(
    app: AppHandle,
    db: State<'_, Database>,
    _document_id: String,
    section_id: String,
) -> Result<(), String> {
    let api_key = get_api_key_from_keyring().map_err(|e| e.to_string())?;

    // Load section + document + session context
    let (session_id, directive, existing_content) = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let section = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
        let document = store::get_document(&conn, &section.document_id)
            .map_err(|e| e.to_string())?;
        (document.session_id, section.directive, section.content)
    };

    let (session_context, note_content, tickets, jira_keys) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        let session = crate::session::store::get(&conn, &session_id)
            .map_err(|e| e.to_string())?;

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

        let mut all_text = note_content.clone();
        all_text.push(' ');
        all_text.push_str(&directive);
        let jira_keys = extract_jira_keys(&all_text);

        (session.context, note_content, tickets, jira_keys)
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

    let (repo_summaries, _mentioned_files) = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let repos = repo_store::list_by_session(&conn, &session_id)
            .map_err(|e| e.to_string())?;

        let summaries: Vec<String> = repos
            .iter()
            .filter_map(|r| {
                repo_tree::generate_summary(std::path::Path::new(&r.local_path))
                    .ok()
                    .map(|s| format!("{} ({})", r.name, s))
            })
            .collect();

        // No @mentions in directives — mentioned_files is always empty for doc generation
        let files: Vec<RepoFileContext> = Vec::new();

        (summaries, files)
    };

    let retrieved_chunks: Vec<RetrievedChunk> = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let repos = repo_store::list_by_session(&conn, &session_id)
            .map_err(|e| e.to_string())?;
        let repo_ids: Vec<String> = repos.iter().map(|r| r.id.clone()).collect();

        let has_chunks: bool = repo_ids.iter().any(|rid| {
            crate::rag::store::count_by_repo(&conn, rid).unwrap_or(0) > 0
        });

        if has_chunks {
            let embedder: State<Embedder> = app.state::<Embedder>();
            let query_texts = vec![directive.clone()];
            match embedder.embed(&query_texts) {
                Ok(embeddings) if !embeddings.is_empty() => {
                    rag_search::hybrid_search(&conn, &embeddings[0], &directive, &repo_ids)
                        .unwrap_or_default()
                }
                _ => vec![],
            }
        } else {
            vec![]
        }
    };

    let (messages, model) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        // Assemble session context exactly like send_message, but with:
        // - doc-specific system prompt (no conversation mode)
        // - directive as the sole user message (no conversation history)
        let mut system_parts = vec![DOC_SYSTEM_PROMPT.to_string()];

        if !session_context.is_empty() {
            system_parts.push(format!("## Session Context\n{session_context}"));
        }
        if !note_content.is_empty() {
            system_parts.push(format!("## Brain Dump Notes\n{note_content}"));
        }
        if !tickets.is_empty() {
            let mut ticket_text = String::from("## Current Tickets\n");
            for (title, ticket_type, priority, description) in &tickets {
                let desc_preview = if description.len() > 100 {
                    let truncated: String = description.chars().take(100).collect();
                    format!("{truncated}...")
                } else {
                    description.clone()
                };
                ticket_text.push_str(&format!(
                    "- {title} ({ticket_type}, {priority}) — {desc_preview}\n"
                ));
            }
            system_parts.push(ticket_text);
        }
        if !jira_issues.is_empty() {
            let mut jira_text = String::from("## Referenced Jira Tickets\n");
            for issue in &jira_issues {
                jira_text.push_str(&format!(
                    "- {} ({}, {}, {}): {}\n",
                    issue.key, issue.issue_type, issue.status, issue.priority, issue.summary
                ));
            }
            system_parts.push(jira_text);
        }
        if !repo_summaries.is_empty() {
            let mut repo_text = String::from("## Attached Repositories\n");
            for summary in &repo_summaries {
                repo_text.push_str(&format!("- {summary}\n"));
            }
            system_parts.push(repo_text);
        }
        if !retrieved_chunks.is_empty() {
            let mut rag_text = String::from(
                "## Retrieved Code Context\nRelevant code snippets from attached repositories:\n\n",
            );
            for chunk in &retrieved_chunks {
                rag_text.push_str(&format!(
                    "### {}/{} (lines {}-{})\n```\n{}\n```\n\n",
                    chunk.repo_name, chunk.file_path, chunk.start_line, chunk.end_line, chunk.content,
                ));
            }
            system_parts.push(rag_text);
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_parts.join("\n\n"),
            },
            ChatMessage {
                role: "user".to_string(),
                content: directive.clone(),
            },
        ];

        let model = settings_store::get(&conn, "llm.model")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| models::DEFAULT_MODEL.to_string());

        (messages, model)
    };

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);

    let cancel = CancellationToken::new();
    {
        let tokens: State<CancellationTokens> = app.state::<CancellationTokens>();
        let mut map = tokens.tokens.lock().unwrap();
        map.insert(section_id.clone(), cancel.clone());
    }

    let app_clone = app.clone();
    let section_id_clone = section_id.clone();

    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        client::stream_completion(&api_key, &model, messages, tx, cancel_clone).await;
    });

    tokio::spawn(async move {
        let mut full_content = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    let _ = app_clone.emit(
                        "doc:chunk",
                        DocChunkPayload {
                            section_id: section_id_clone.clone(),
                            content: text,
                        },
                    );
                }
                StreamEvent::Done(text) => {
                    full_content = text;
                    let _ = app_clone.emit(
                        "doc:done",
                        DocDonePayload {
                            section_id: section_id_clone.clone(),
                            full_content: full_content.clone(),
                        },
                    );
                    break;
                }
                StreamEvent::Error(msg) => {
                    let _ = app_clone.emit(
                        "doc:error",
                        DocErrorPayload {
                            section_id: section_id_clone.clone(),
                            message: msg,
                        },
                    );
                    return;
                }
            }
        }

        app_clone
            .state::<CancellationTokens>()
            .tokens
            .lock()
            .ok()
            .map(|mut m| m.remove(&section_id_clone));

        if !full_content.is_empty() {
            let db: State<Database> = app_clone.state::<Database>();
            if let Ok(conn) = db.conn() {
                // Save previous content as a version before overwriting
                if let Err(e) = store::save_version(&conn, &section_id_clone, &existing_content) {
                    tracing::warn!("Failed to save section version: {e}");
                }
                if let Err(e) =
                    store::update_section_content(&conn, &section_id_clone, &full_content)
                {
                    tracing::error!("Failed to save generated section content: {e}");
                    let _ = app_clone.emit(
                        "doc:error",
                        DocErrorPayload {
                            section_id: section_id_clone,
                            message: "Content generated but failed to save — please try again"
                                .to_string(),
                        },
                    );
                };
            };
        }
    });

    Ok(())
}
