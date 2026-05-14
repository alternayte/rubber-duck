# Settings + OpenRouter LLM Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a settings system and OpenRouter-based LLM integration so the duck chat can send messages, stream responses, and persist conversations.

**Architecture:** New `settings/` Rust module for key-value settings in SQLite + API key in OS keychain. New `llm/` module with OpenRouter client, SSE streaming bridge to Tauri events, and context assembly from session data. Frontend settings dialog with shadcn components and Jotai atoms.

**Tech Stack:** Rust (keyring crate, reqwest SSE streaming, futures), React (shadcn Dialog/Select/Label, Jotai atoms)

**Spec:** `docs/superpowers/specs/2026-05-14-settings-llm-integration-design.md`

---

## File Map

### Rust — Create

| File | Responsibility |
|------|---------------|
| `src-tauri/migrations/003_add_settings.sql` | Settings table schema |
| `src-tauri/src/settings/mod.rs` | Module declarations |
| `src-tauri/src/settings/store.rs` | Settings key-value CRUD + tests |
| `src-tauri/src/settings/commands.rs` | Tauri commands for settings + API key |
| `src-tauri/src/llm/models.rs` | Curated model list with metadata |
| `src-tauri/src/llm/context.rs` | Context assembly from session data + tests |
| `src-tauri/src/llm/client.rs` | OpenRouter HTTP client with SSE streaming |
| `src-tauri/src/llm/streaming.rs` | SSE → Tauri event bridge + send_message command |

### Rust — Modify

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `keyring`, `futures` deps |
| `src-tauri/src/db.rs` | Add `003_add_settings` migration |
| `src-tauri/src/error.rs` | Add `Keyring` error variant |
| `src-tauri/src/lib.rs` | Add `settings` module, register new commands |
| `src-tauri/src/llm/mod.rs` | Add sub-module declarations |

### Frontend — Create

| File | Responsibility |
|------|---------------|
| `src/features/settings/settings.atoms.ts` | Jotai atoms for apiKeySet, selectedModel |
| `src/features/settings/SettingsDialog.tsx` | Settings modal with API key + model selector |

### Frontend — Modify

| File | Change |
|------|--------|
| `src/features/session/SessionSidebar.tsx` | Add gear icon to open settings |
| `src/App.tsx` | Mount SettingsDialog |

### shadcn Components to Add

`dialog`, `select`, `label` — via `bunx shadcn@latest add dialog select label --yes --overwrite`

---

## Task 1: Settings migration + store

**Files:**
- Create: `src-tauri/migrations/003_add_settings.sql`
- Create: `src-tauri/src/settings/mod.rs`
- Create: `src-tauri/src/settings/store.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the migration SQL**

Create `src-tauri/migrations/003_add_settings.sql`:
```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO settings (key, value, category) VALUES ('llm.model', 'deepseek/deepseek-chat-v4-0324:free', 'llm');
INSERT INTO settings (key, value, category) VALUES ('llm.api_key_ref', '', 'llm');
```

- [ ] **Step 2: Register the migration in db.rs**

Add to the `MIGRATIONS` const in `src-tauri/src/db.rs`:
```rust
Migration {
    name: "003_add_settings",
    sql: include_str!("../migrations/003_add_settings.sql"),
},
```

- [ ] **Step 3: Write failing tests for settings store**

Create `src-tauri/src/settings/store.rs` with tests at the bottom:
```rust
use rusqlite::{params, Connection};
use crate::error::AppResult;

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    todo!()
}

pub fn set(conn: &Connection, key: &str, value: &str, category: &str) -> AppResult<()> {
    todo!()
}

pub fn get_by_category(conn: &Connection, category: &str) -> AppResult<Vec<(String, String)>> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn get_returns_default_setting() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let value = get(&conn, "llm.model").unwrap();
        assert_eq!(value, Some("deepseek/deepseek-chat-v4-0324:free".to_string()));
    }

    #[test]
    fn get_returns_none_for_missing_key() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let value = get(&conn, "nonexistent").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn set_updates_existing_key() {
        let db = test_db();
        let conn = db.conn().unwrap();
        set(&conn, "llm.model", "openai/gpt-4o", "llm").unwrap();
        let value = get(&conn, "llm.model").unwrap();
        assert_eq!(value, Some("openai/gpt-4o".to_string()));
    }

    #[test]
    fn set_creates_new_key() {
        let db = test_db();
        let conn = db.conn().unwrap();
        set(&conn, "general.theme", "dark", "general").unwrap();
        let value = get(&conn, "general.theme").unwrap();
        assert_eq!(value, Some("dark".to_string()));
    }

    #[test]
    fn get_by_category_returns_matching() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let llm_settings = get_by_category(&conn, "llm").unwrap();
        assert_eq!(llm_settings.len(), 2);
    }
}
```

- [ ] **Step 4: Run tests, verify they fail**

Run: `cd src-tauri && cargo test settings::store::tests -v`
Expected: FAIL (todo! panics)

- [ ] **Step 5: Implement the store functions**

Replace the `todo!()` calls in `src-tauri/src/settings/store.rs`:
```rust
pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    );
    match result {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str, category: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value, category, updated_at)
         VALUES (?1, ?2, ?3, datetime('now'))
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now')",
        params![key, value, category],
    )?;
    Ok(())
}

pub fn get_by_category(conn: &Connection, category: &str) -> AppResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM settings WHERE category = ?1 ORDER BY key",
    )?;
    let rows = stmt
        .query_map(params![category], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

- [ ] **Step 6: Create mod.rs and wire into lib.rs**

Create `src-tauri/src/settings/mod.rs`:
```rust
pub mod commands;
pub mod store;
```

Add to `src-tauri/src/lib.rs` after `mod ticket;`:
```rust
mod settings;
```

Create an empty `src-tauri/src/settings/commands.rs`:
```rust
// Tauri commands added in Task 2
```

- [ ] **Step 7: Run tests, verify they pass**

Run: `cd src-tauri && cargo test -v`
Expected: All tests pass (previous 20 + 5 new = 25)

- [ ] **Step 8: Commit**

```bash
git add src-tauri/migrations/003_add_settings.sql src-tauri/src/settings/ src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat: add settings table with key-value store"
```

---

## Task 2: Settings Tauri commands + keyring

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/error.rs`
- Create: `src-tauri/src/settings/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add keyring dependency**

Add to `[dependencies]` in `src-tauri/Cargo.toml`:
```toml
keyring = { version = "3", features = ["apple-native", "windows-native", "sync-secret-service"] }
```

- [ ] **Step 2: Add Keyring error variant**

Add to the `AppError` enum in `src-tauri/src/error.rs`:
```rust
#[error("Keyring error: {0}")]
Keyring(String),
```

- [ ] **Step 3: Write the settings commands**

Replace `src-tauri/src/settings/commands.rs`:
```rust
use serde::Serialize;
use tauri::State;

use crate::db::Database;
use crate::error::AppError;

use super::store;

#[derive(Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub context_window: u32,
}

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

pub fn get_api_key_from_keyring() -> Result<String, AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()))?;
    entry
        .get_password()
        .map_err(|e| AppError::Keyring(e.to_string()))
}

#[tauri::command]
pub fn get_available_models() -> Vec<ModelInfo> {
    crate::llm::models::MODELS.to_vec()
}
```

Note: `get_available_models` references `llm::models` which we create in Task 3. This command will be wired in after Task 3.

- [ ] **Step 4: Register settings commands in lib.rs**

Add to `src-tauri/src/lib.rs`:
```rust
use settings::commands::*;
```

Add to the `invoke_handler` list:
```rust
get_setting,
set_setting,
set_api_key,
has_api_key,
```

(`get_available_models` added after Task 3 when `llm::models` exists)

- [ ] **Step 5: Run cargo check**

Run: `cd src-tauri && cargo check`
Expected: Compiles (warnings about unused `get_api_key_from_keyring` are OK — used in Task 7)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/error.rs src-tauri/src/settings/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add settings commands with keyring API key storage"
```

---

## Task 3: LLM models list

**Files:**
- Create: `src-tauri/src/llm/models.rs`
- Modify: `src-tauri/src/llm/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create the curated model list**

Create `src-tauri/src/llm/models.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub context_window: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "deepseek/deepseek-chat-v4-0324:free",
        name: "DeepSeek v4 Flash",
        context_window: 131072,
    },
    ModelInfo {
        id: "deepseek/deepseek-chat",
        name: "DeepSeek v4",
        context_window: 131072,
    },
    ModelInfo {
        id: "anthropic/claude-sonnet-4",
        name: "Claude Sonnet 4",
        context_window: 200000,
    },
    ModelInfo {
        id: "anthropic/claude-haiku-4",
        name: "Claude Haiku 4",
        context_window: 200000,
    },
    ModelInfo {
        id: "openai/gpt-4o",
        name: "GPT-4o",
        context_window: 128000,
    },
    ModelInfo {
        id: "openai/gpt-4o-mini",
        name: "GPT-4o Mini",
        context_window: 128000,
    },
    ModelInfo {
        id: "meta-llama/llama-4-scout",
        name: "Llama 4 Scout",
        context_window: 512000,
    },
    ModelInfo {
        id: "qwen/qwen3-235b",
        name: "Qwen 3 235B",
        context_window: 131072,
    },
];

pub const DEFAULT_MODEL: &str = "deepseek/deepseek-chat-v4-0324:free";
```

- [ ] **Step 2: Update llm/mod.rs**

Replace `src-tauri/src/llm/mod.rs`:
```rust
pub mod client;
pub mod context;
pub mod models;
pub mod streaming;
```

Create empty stubs for files that don't exist yet:

`src-tauri/src/llm/client.rs`:
```rust
// OpenRouter client — implemented in Task 5
```

`src-tauri/src/llm/context.rs`:
```rust
// Context assembly — implemented in Task 4
```

`src-tauri/src/llm/streaming.rs`:
```rust
// SSE → Tauri event bridge — implemented in Task 6
```

- [ ] **Step 3: Wire get_available_models into lib.rs**

Add `get_available_models` to the `invoke_handler` list in `src-tauri/src/lib.rs`.

Update `settings/commands.rs` to use the shared type. Remove the local `ModelInfo` struct and import from `llm::models`:
```rust
use crate::llm::models::ModelInfo;

#[tauri::command]
pub fn get_available_models() -> Vec<ModelInfo> {
    crate::llm::models::MODELS.to_vec()
}
```

- [ ] **Step 4: Run cargo check**

Run: `cd src-tauri && cargo check`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/llm/ src-tauri/src/settings/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add curated OpenRouter model list"
```

---

## Task 4: Context assembly

**Files:**
- Create: `src-tauri/src/llm/context.rs` (replace stub)

- [ ] **Step 1: Write failing tests for context assembly**

Replace `src-tauri/src/llm/context.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

const SYSTEM_PROMPT: &str = "You are a technical planning assistant embedded in a local brainstorming tool called rubber-duck. Your job is to help the user think through technical problems and produce well-structured work items.

When asked to create tickets, produce structured JSON that the app can parse. When asked to review or improve, be specific and actionable.";

const MAX_CONVERSATION_MESSAGES: usize = 40;

pub fn assemble_context(
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    conversation: &[(String, String)],
) -> Vec<ChatMessage> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_message_includes_prompt() {
        let messages = assemble_context("", "", &[], &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("rubber-duck"));
    }

    #[test]
    fn includes_session_context() {
        let messages = assemble_context("We are migrating the auth service", "", &[], &[]);
        assert!(messages[0].content.contains("migrating the auth service"));
    }

    #[test]
    fn includes_note_content() {
        let messages = assemble_context("", "# My brainstorm\nSome ideas here", &[], &[]);
        assert!(messages[0].content.contains("My brainstorm"));
    }

    #[test]
    fn includes_tickets() {
        let tickets = vec![
            ("Fix login".to_string(), "Task".to_string(), "High".to_string(), "Auth is broken".to_string()),
        ];
        let messages = assemble_context("", "", &tickets, &[]);
        assert!(messages[0].content.contains("Fix login"));
        assert!(messages[0].content.contains("Task"));
    }

    #[test]
    fn includes_conversation_as_separate_messages() {
        let conversation = vec![
            ("User".to_string(), "Break this into tickets".to_string()),
            ("Assistant".to_string(), "Here are 3 tickets...".to_string()),
        ];
        let messages = assemble_context("", "", &[], &conversation);
        assert_eq!(messages.len(), 3); // system + 2 conversation
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    #[test]
    fn caps_conversation_at_40_messages() {
        let conversation: Vec<(String, String)> = (0..60)
            .map(|i| {
                let role = if i % 2 == 0 { "User" } else { "Assistant" };
                (role.to_string(), format!("Message {i}"))
            })
            .collect();
        let messages = assemble_context("", "", &[], &conversation);
        // system + 40 conversation = 41
        assert_eq!(messages.len(), 41);
        // Should keep the LAST 40 (messages 20-59)
        assert!(messages[1].content.contains("Message 20"));
    }
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cd src-tauri && cargo test llm::context::tests -v`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement context assembly**

Replace the `todo!()` in `assemble_context`:
```rust
pub fn assemble_context(
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)], // (title, type, priority, description)
    conversation: &[(String, String)], // (role, content)
) -> Vec<ChatMessage> {
    let mut system_parts = vec![SYSTEM_PROMPT.to_string()];

    if !session_context.is_empty() {
        system_parts.push(format!("## Session Context\n{session_context}"));
    }

    if !note_content.is_empty() {
        system_parts.push(format!("## Brain Dump Notes\n{note_content}"));
    }

    if !tickets.is_empty() {
        let mut ticket_text = String::from("## Current Tickets\n");
        for (title, ticket_type, priority, description) in tickets {
            let desc_preview = if description.len() > 100 {
                format!("{}...", &description[..100])
            } else {
                description.clone()
            };
            ticket_text.push_str(&format!("- {title} ({ticket_type}, {priority}) — {desc_preview}\n"));
        }
        system_parts.push(ticket_text);
    }

    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_parts.join("\n\n"),
    }];

    let conv_slice = if conversation.len() > MAX_CONVERSATION_MESSAGES {
        &conversation[conversation.len() - MAX_CONVERSATION_MESSAGES..]
    } else {
        conversation
    };

    for (role, content) in conv_slice {
        messages.push(ChatMessage {
            role: match role.as_str() {
                "User" => "user",
                "Assistant" => "assistant",
                _ => "system",
            }
            .to_string(),
            content: content.clone(),
        });
    }

    messages
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cd src-tauri && cargo test llm::context::tests -v`
Expected: All 6 tests pass

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/llm/context.rs
git commit -m "feat: add LLM context assembly with 40-message conversation cap"
```

---

## Task 5: OpenRouter HTTP client

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/llm/client.rs` (replace stub)

- [ ] **Step 1: Add futures dependency**

Add to `[dependencies]` in `src-tauri/Cargo.toml`:
```toml
futures = "0.3"
```

- [ ] **Step 2: Implement the OpenRouter client**

Replace `src-tauri/src/llm/client.rs`:
```rust
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::{AppError, AppResult};

use super::context::ChatMessage;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Debug, Serialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug)]
pub enum StreamEvent {
    Chunk(String),
    Done(String),
    Error(String),
}

pub async fn stream_completion(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: mpsc::Sender<StreamEvent>,
) {
    let result = stream_inner(api_key, model, messages, &tx).await;
    if let Err(e) = result {
        let _ = tx.send(StreamEvent::Error(e.to_string())).await;
    }
}

async fn stream_inner(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    let client = Client::new();
    let response = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("HTTP-Referer", "rubber-duck")
        .header("X-Title", "rubber-duck")
        .json(&CompletionRequest {
            model: model.to_string(),
            messages,
            stream: true,
        })
        .send()
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(AppError::Other(format!("OpenRouter {status}: {body}")));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_content = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Other(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                return Ok(());
            }

            match serde_json::from_str::<StreamResponse>(data) {
                Ok(parsed) => {
                    if let Some(choice) = parsed.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            full_content.push_str(content);
                            let _ = tx.send(StreamEvent::Chunk(content.clone())).await;
                        }
                        if choice.finish_reason.is_some() {
                            let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                            return Ok(());
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }

    let _ = tx.send(StreamEvent::Done(full_content)).await;
    Ok(())
}
```

- [ ] **Step 3: Run cargo check**

Run: `cd src-tauri && cargo check`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/llm/client.rs
git commit -m "feat: add OpenRouter streaming HTTP client"
```

---

## Task 6: Streaming bridge + send_message command

**Files:**
- Create: `src-tauri/src/llm/streaming.rs` (replace stub)
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement the streaming bridge and send_message command**

Replace `src-tauri/src/llm/streaming.rs`:
```rust
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::db::Database;
use crate::error::AppError;
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
) -> Result<(), String> {
    let api_key = get_api_key_from_keyring().map_err(|e| e.to_string())?;

    let (messages, model) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        let user_msg_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, 'User', ?3)",
            params![user_msg_id, session_id, content],
        )
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

        let messages = context::assemble_context(
            &session.context,
            &note_content,
            &tickets,
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
            let db = app_clone.state::<Database>();
            if let Ok(conn) = db.conn() {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, 'Assistant', ?3)",
                    params![id, db_clone_session_id, full_content],
                );
            }
        }
    });

    Ok(())
}
```

- [ ] **Step 2: Register send_message in lib.rs**

Add to the `invoke_handler` list in `src-tauri/src/lib.rs`:
```rust
llm::streaming::send_message,
get_available_models,
```

Also add the import for `send_message` — since it's in a submodule, reference it directly in `generate_handler!` as shown above.

- [ ] **Step 3: Run cargo check**

Run: `cd src-tauri && cargo check`
Expected: Compiles

- [ ] **Step 4: Run all tests**

Run: `cd src-tauri && cargo test -v`
Expected: All tests pass (25 settings + context tests)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/llm/streaming.rs src-tauri/src/lib.rs
git commit -m "feat: add send_message command with SSE streaming bridge"
```

---

## Task 7: Settings dialog frontend

**Files:**
- Install: shadcn dialog, select, label components
- Create: `src/features/settings/settings.atoms.ts`
- Create: `src/features/settings/SettingsDialog.tsx`

- [ ] **Step 1: Install shadcn components**

```bash
bunx shadcn@latest add dialog select label --yes --overwrite
```

- [ ] **Step 2: Create settings atoms**

Create `src/features/settings/settings.atoms.ts`:
```typescript
import { atom } from "jotai";

export const apiKeySetAtom = atom(false);
export const selectedModelAtom = atom("deepseek/deepseek-chat-v4-0324:free");
export const settingsOpenAtom = atom(false);
```

- [ ] **Step 3: Create the settings dialog**

Create `src/features/settings/SettingsDialog.tsx`:
```tsx
import { useEffect, useState } from "react";
import { useAtom, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  apiKeySetAtom,
  selectedModelAtom,
  settingsOpenAtom,
} from "./settings.atoms";

interface ModelInfo {
  id: string;
  name: string;
  context_window: number;
}

export function SettingsDialog() {
  const [open, setOpen] = useAtom(settingsOpenAtom);
  const [apiKeySet, setApiKeySet] = useAtom(apiKeySetAtom);
  const [selectedModel, setSelectedModel] = useAtom(selectedModelAtom);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      invoke<boolean>("has_api_key").then(setApiKeySet);
      invoke<ModelInfo[]>("get_available_models").then(setModels);
      invoke<string | null>("get_setting", { key: "llm.model" }).then(
        (val) => {
          if (val) setSelectedModel(val);
        },
      );
    }
  }, [open]);

  async function handleSaveApiKey() {
    if (!apiKeyInput.trim()) return;
    setSaving(true);
    await invoke("set_api_key", { key: apiKeyInput.trim() });
    setApiKeySet(true);
    setApiKeyInput("");
    setShowKey(false);
    setSaving(false);
  }

  async function handleModelChange(modelId: string) {
    setSelectedModel(modelId);
    await invoke("set_setting", {
      key: "llm.model",
      value: modelId,
      category: "llm",
    });
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div className="space-y-6 py-4">
          <div className="space-y-2">
            <Label>OpenRouter API Key</Label>
            <div className="flex items-center gap-2">
              {apiKeySet && (
                <span className="text-xs text-green-500">✓ Key saved</span>
              )}
              {!apiKeySet && (
                <span className="text-xs text-destructive-foreground">
                  No key set
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <Input
                type={showKey ? "text" : "password"}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder="sk-or-..."
                className="flex-1"
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowKey(!showKey)}
              >
                {showKey ? "Hide" : "Show"}
              </Button>
            </div>
            <Button
              size="sm"
              onClick={handleSaveApiKey}
              disabled={!apiKeyInput.trim() || saving}
            >
              {saving ? "Saving..." : "Save Key"}
            </Button>
          </div>

          <div className="space-y-2">
            <Label>Model</Label>
            <Select value={selectedModel} onValueChange={handleModelChange}>
              <SelectTrigger>
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {models.map((model) => (
                  <SelectItem key={model.id} value={model.id}>
                    {model.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 4: Run tsc to verify types**

Run: `npx tsc --noEmit`
Expected: Clean (no errors)

- [ ] **Step 5: Commit**

```bash
git add src/components/ui/ src/features/settings/
git commit -m "feat: add settings dialog with API key and model selector"
```

---

## Task 8: Wire settings into sidebar and App

**Files:**
- Modify: `src/features/session/SessionSidebar.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Add gear icon to sidebar**

Add to `src/features/session/SessionSidebar.tsx`:

Import at top:
```tsx
import { useSetAtom } from "jotai";
import { settingsOpenAtom } from "@/features/settings/settings.atoms";
import { Settings } from "lucide-react";
```

Add inside the component:
```tsx
const setSettingsOpen = useSetAtom(settingsOpenAtom);
```

Replace the sidebar footer `<div className="border-t border-border p-2">` section:
```tsx
<div className="border-t border-border p-2 space-y-1">
  <Button
    variant="secondary"
    size="sm"
    className="w-full"
    onClick={() => setIsCreating(true)}
  >
    + New Session
  </Button>
  <Button
    variant="ghost"
    size="sm"
    className="w-full text-muted-foreground"
    onClick={() => setSettingsOpen(true)}
  >
    <Settings className="size-4" />
    Settings
  </Button>
</div>
```

- [ ] **Step 2: Mount SettingsDialog in App.tsx**

Add import to `src/App.tsx`:
```tsx
import { SettingsDialog } from "@/features/settings/SettingsDialog";
```

Add `<SettingsDialog />` just before the closing `</div>` of the root element.

- [ ] **Step 3: Load initial settings state on app start**

Add to `src/App.tsx` inside the `App` function:
```tsx
import { useEffect } from "react";
import { useSetAtom } from "jotai";
import { apiKeySetAtom, selectedModelAtom } from "@/features/settings/settings.atoms";

// Inside App():
const setApiKeySet = useSetAtom(apiKeySetAtom);
const setSelectedModel = useSetAtom(selectedModelAtom);

useEffect(() => {
  invoke<boolean>("has_api_key").then(setApiKeySet);
  invoke<string | null>("get_setting", { key: "llm.model" }).then((val) => {
    if (val) setSelectedModel(val);
  });
}, []);
```

Add the `invoke` import if not already present:
```tsx
import { invoke } from "@tauri-apps/api/core";
```

- [ ] **Step 4: Run tsc and verify**

Run: `npx tsc --noEmit`
Expected: Clean

- [ ] **Step 5: Run all Rust tests**

Run: `cd src-tauri && cargo test -v`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/features/session/SessionSidebar.tsx src/App.tsx
git commit -m "feat: wire settings dialog into sidebar with gear icon"
```

---

## Task 9: Update PLAN.md

**Files:**
- Modify: `docs/PLAN.md`

- [ ] **Step 1: Check off Task 1.5 items**

Update the Task 1.5 section in `docs/PLAN.md`:
```markdown
### Task 1.5 — LLM integration core
- [x] OpenRouter client with SSE streaming (`llm/client.rs`) — no provider trait
- [x] Context assembly: system prompt + notes + tickets + last 40 messages (`llm/context.rs`)
- [x] SSE → Tauri event bridge (`llm/streaming.rs`): `llm:chunk`, `llm:done`, `llm:error`
- [x] Settings infrastructure: SQLite table + OS keychain for API key (`settings/`)
- [x] Settings UI: dialog with API key input + model selector (shadcn + Jotai)
- [x] Curated model list (8 models, default DeepSeek v4 Flash)
- [x] `send_message` command: saves to DB → assembles context → streams response → saves result
```

- [ ] **Step 2: Commit**

```bash
git add docs/PLAN.md
git commit -m "docs: check off Task 1.5 in plan"
```
