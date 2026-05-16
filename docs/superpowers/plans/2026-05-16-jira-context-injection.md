# Jira Ticket Context Injection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect Jira ticket IDs (e.g. `FRONT-42`) in notes and chat messages, fetch ticket details from Jira, inject them into the LLM context, and make ticket IDs clickable links in markdown preview.

**Architecture:** New `get_issue` method on JiraClient, a regex-based `extract_jira_keys` function, modified `send_message` that fetches referenced Jira issues before assembling context, modified `assemble_context` that includes a Jira tickets section, and a shared `JiraLinkedText` React component for clickable ticket IDs in markdown.

**Tech Stack:** Rust (reqwest, regex, mockito), React 19 (react-markdown custom components), Jotai, @tauri-apps/plugin-opener

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/Cargo.toml` | Add `regex` dependency |
| Modify | `src-tauri/src/jira/model.rs` | Add `JiraIssueContext`, `JiraIssueResponse` structs |
| Modify | `src-tauri/src/jira/client.rs` | Add `get_issue` method + `extract_jira_keys` function + tests |
| Modify | `src-tauri/src/jira/commands.rs` | Add `fetch_jira_issues` Tauri command |
| Modify | `src-tauri/src/lib.rs` | Register `fetch_jira_issues` command |
| Modify | `src-tauri/src/llm/context.rs` | Add `jira_issues` param to `assemble_context` + tests |
| Modify | `src-tauri/src/llm/streaming.rs` | Extract Jira keys, fetch issues, pass to context |
| Create | `src/components/JiraLinkedText.tsx` | Shared component: replaces ticket IDs with clickable links |
| Modify | `src/features/settings/settings.atoms.ts` | Add `jiraBaseUrlAtom` |
| Modify | `src/App.tsx` | Load Jira base URL on app start |
| Modify | `src/features/session/DumpView.tsx` | Use `JiraLinkedText` in markdown preview |
| Modify | `src/features/chat/ChatPanel.tsx` | Use `JiraLinkedText` in assistant messages |

---

### Task 1: Add `get_issue` to JiraClient + `extract_jira_keys`

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/jira/model.rs`
- Modify: `src-tauri/src/jira/client.rs`

- [ ] **Step 1: Add regex dependency**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
regex = "1"
```

- [ ] **Step 2: Add model structs**

In `src-tauri/src/jira/model.rs`, add at the end:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueContext {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub priority: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueResponse {
    pub key: String,
    pub fields: JiraIssueFields,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueFields {
    pub summary: String,
    pub status: JiraNameField,
    pub issuetype: JiraNameField,
    pub priority: Option<JiraNameField>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JiraNameField {
    pub name: String,
}
```

- [ ] **Step 3: Write failing tests for `extract_jira_keys`**

In `src-tauri/src/jira/client.rs`, add at the bottom of the `tests` module:

```rust
#[test]
fn extract_jira_keys_finds_patterns() {
    let text = "Check FRONT-42 and INFRA-7, also mentioned FRONT-42 again";
    let keys = extract_jira_keys(text);
    assert_eq!(keys, vec!["FRONT-42", "INFRA-7"]);
}

#[test]
fn extract_jira_keys_empty_for_no_matches() {
    let keys = extract_jira_keys("no ticket ids here");
    assert!(keys.is_empty());
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cd src-tauri && cargo test extract_jira_keys --lib 2>&1`
Expected: Compilation error — `extract_jira_keys` does not exist.

- [ ] **Step 5: Implement `extract_jira_keys`**

In `src-tauri/src/jira/client.rs`, add at the top of the file (after the existing imports):

```rust
use regex::Regex;
use std::collections::HashSet;
```

Then add this function after the `impl JiraClient` block (it's a free function, not a method):

```rust
pub fn extract_jira_keys(text: &str) -> Vec<String> {
    let re = Regex::new(r"[A-Z][A-Z0-9]+-\d+").unwrap();
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    for m in re.find_iter(text) {
        let key = m.as_str().to_string();
        if seen.insert(key.clone()) {
            keys.push(key);
        }
    }
    keys
}
```

- [ ] **Step 6: Run extract tests to verify they pass**

Run: `cd src-tauri && cargo test extract_jira_keys --lib 2>&1`
Expected: 2 tests pass.

- [ ] **Step 7: Write failing test for `get_issue`**

In `src-tauri/src/jira/client.rs`, add to the `tests` module:

```rust
#[tokio::test]
async fn get_issue_success() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/issue/FRONT-42?fields=summary,status,issuetype,priority,description")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"key":"FRONT-42","fields":{"summary":"Fix login timeout","status":{"name":"In Progress"},"issuetype":{"name":"Bug"},"priority":{"name":"High"},"description":"When users attempt to log in with SSO the request times out"}}"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), JiraAuth::Basic {
        email: "test@example.com".to_string(),
        api_token: "token".to_string(),
    }).unwrap();
    let issue = client.get_issue("FRONT-42").await.unwrap();

    assert_eq!(issue.key, "FRONT-42");
    assert_eq!(issue.summary, "Fix login timeout");
    assert_eq!(issue.status, "In Progress");
    assert_eq!(issue.issue_type, "Bug");
    assert_eq!(issue.priority, "High");
    assert!(issue.description.contains("SSO"));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_issue_not_found() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/issue/NOPE-1?fields=summary,status,issuetype,priority,description")
        .with_status(404)
        .with_body(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), JiraAuth::Basic {
        email: "test@example.com".to_string(),
        api_token: "token".to_string(),
    }).unwrap();
    let result = client.get_issue("NOPE-1").await;

    assert!(result.is_err());
    mock.assert_async().await;
}
```

- [ ] **Step 8: Implement `get_issue`**

In `src-tauri/src/jira/client.rs`, add the imports for the new types to the existing use statement:

```rust
use super::model::{
    CreateIssueFields, CreateIssueRequest, IssueTypeRef, ProjectRef,
    JiraAuth, JiraErrorResponse, JiraUser, CreateIssueResponse, JiraProject,
    JiraIssueContext, JiraIssueResponse,
};
```

Then add this method to the `impl JiraClient` block, after `get_projects`:

```rust
pub async fn get_issue(&self, issue_key: &str) -> AppResult<JiraIssueContext> {
    let url = format!(
        "{}/rest/api/2/issue/{}?fields=summary,status,issuetype,priority,description",
        self.base_url, issue_key
    );
    let response = self
        .apply_auth(self.client.get(&url))
        .send()
        .await?;

    if response.status().is_success() {
        let issue: JiraIssueResponse = response.json().await?;
        let desc = issue.fields.description.unwrap_or_default();
        let description = if desc.chars().count() > 500 {
            let truncated: String = desc.chars().take(500).collect();
            format!("{truncated}...")
        } else {
            desc
        };
        return Ok(JiraIssueContext {
            key: issue.key,
            summary: issue.fields.summary,
            status: issue.fields.status.name,
            issue_type: issue.fields.issuetype.name,
            priority: issue.fields.priority.map(|p| p.name).unwrap_or_else(|| "None".to_string()),
            description,
        });
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = parse_jira_error(&body, status.as_u16());
    Err(AppError::Other(message))
}
```

- [ ] **Step 9: Run all jira tests**

Run: `cd src-tauri && cargo test jira --lib 2>&1`
Expected: All 13 jira tests pass (9 existing + 2 extract + 2 get_issue).

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/jira/model.rs src-tauri/src/jira/client.rs
git commit -m "feat: add get_issue and extract_jira_keys to Jira client"
```

---

### Task 2: Add `fetch_jira_issues` Tauri command

**Files:**
- Modify: `src-tauri/src/jira/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the Tauri command**

In `src-tauri/src/jira/commands.rs`, add the import for `JiraIssueContext`:

```rust
use super::model::{JiraAuth, JiraUser, JiraProject, JiraIssueContext};
```

Then add after `get_jira_projects`:

```rust
#[tauri::command]
pub async fn fetch_jira_issues(
    db: State<'_, Database>,
    keys: Vec<String>,
) -> Result<Vec<JiraIssueContext>, String> {
    let credentials = get_jira_credentials(&db);
    let (base_url, auth) = match credentials {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };
    let client = match JiraClient::new(&base_url, auth) {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let mut results = Vec::new();
    for key in &keys {
        match client.get_issue(key).await {
            Ok(issue) => results.push(issue),
            Err(e) => tracing::warn!("Failed to fetch Jira issue {key}: {e}"),
        }
    }
    Ok(results)
}
```

- [ ] **Step 2: Register in `lib.rs`**

In `src-tauri/src/lib.rs`, add `fetch_jira_issues` to the `invoke_handler` list, after `get_jira_projects`:

```rust
get_jira_projects,
fetch_jira_issues,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/jira/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add fetch_jira_issues Tauri command"
```

---

### Task 3: Modify `assemble_context` to include Jira issues

**Files:**
- Modify: `src-tauri/src/llm/context.rs`

- [ ] **Step 1: Write failing test**

In `src-tauri/src/llm/context.rs`, add to the `tests` module:

```rust
#[test]
fn includes_jira_issues_in_context() {
    let jira_issues = vec![
        JiraIssueContext {
            key: "FRONT-42".to_string(),
            summary: "Fix login timeout".to_string(),
            status: "In Progress".to_string(),
            issue_type: "Bug".to_string(),
            priority: "High".to_string(),
            description: "SSO times out after 30s".to_string(),
        },
    ];
    let messages = assemble_context(&ChatMode::Assist, "", "", &[], &jira_issues, &[]);
    assert!(messages[0].content.contains("Referenced Jira Tickets"));
    assert!(messages[0].content.contains("FRONT-42"));
    assert!(messages[0].content.contains("Fix login timeout"));
    assert!(messages[0].content.contains("SSO times out"));
}

#[test]
fn empty_jira_issues_omits_section() {
    let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[]);
    assert!(!messages[0].content.contains("Referenced Jira Tickets"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test context --lib -- jira 2>&1`
Expected: Compilation error — `assemble_context` doesn't accept `jira_issues` parameter.

- [ ] **Step 3: Update `assemble_context` signature and implementation**

In `src-tauri/src/llm/context.rs`, add the import at the top:

```rust
use crate::jira::model::JiraIssueContext;
```

Then update the function signature from:

```rust
pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    conversation: &[(String, String)],
) -> Vec<ChatMessage> {
```

to:

```rust
pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    jira_issues: &[JiraIssueContext],
    conversation: &[(String, String)],
) -> Vec<ChatMessage> {
```

After the `tickets` block (after `system_parts.push(ticket_text);`), add:

```rust
    if !jira_issues.is_empty() {
        let mut jira_text = String::from("## Referenced Jira Tickets\n");
        for issue in jira_issues {
            let desc_preview = if issue.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", issue.description)
            };
            jira_text.push_str(&format!(
                "- {} ({}, {}, {}): {}{}\n",
                issue.key, issue.issue_type, issue.status, issue.priority, issue.summary, desc_preview
            ));
        }
        system_parts.push(jira_text);
    }
```

- [ ] **Step 4: Fix existing tests**

All existing tests call `assemble_context` without the new `jira_issues` parameter. Update every test call to add `&[]` for the new parameter. The parameter goes between `tickets` and `conversation`:

Every existing call like:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[])
```
becomes:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &[])
```

And calls with tickets:
```rust
assemble_context(&ChatMode::Assist, "", "", &tickets, &[])
```
become:
```rust
assemble_context(&ChatMode::Assist, "", "", &tickets, &[], &[])
```

And calls with conversation:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &conversation)
```
become:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &conversation)
```

There are 8 existing test calls to update. Each just needs `&[]` inserted as the 5th argument.

- [ ] **Step 5: Run all context tests**

Run: `cd src-tauri && cargo test context --lib 2>&1`
Expected: All 10 tests pass (8 existing + 2 new).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/context.rs
git commit -m "feat: add Jira issues section to LLM context assembly"
```

---

### Task 4: Modify `send_message` to extract and fetch Jira issues

**Files:**
- Modify: `src-tauri/src/llm/streaming.rs`

- [ ] **Step 1: Add imports**

At the top of `src-tauri/src/llm/streaming.rs`, add:

```rust
use crate::jira::client::{extract_jira_keys, JiraClient};
use crate::jira::commands::get_jira_credentials;
use crate::jira::model::JiraIssueContext;
```

- [ ] **Step 2: Add Jira fetch between context load and assembly**

In the `send_message` function, find the block where `messages` is assembled (around line 83-89):

```rust
        let messages = context::assemble_context(
            &chat_mode,
            &session.context,
            &note_content,
            &tickets,
            &conversation,
        );
```

Replace with:

```rust
        let jira_keys = {
            let mut all_text = note_content.clone();
            all_text.push(' ');
            all_text.push_str(&content);
            extract_jira_keys(&all_text)
        };

        (session.context.clone(), note_content, tickets, conversation, jira_keys)
    };

    let jira_issues = if !jira_keys.is_empty() {
        match get_jira_credentials_from_db(&db) {
            Ok((base_url, auth)) => {
                match JiraClient::new(&base_url, auth) {
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
                }
            }
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
```

This requires restructuring the existing block. The full approach:

1. The first `let ... = { let conn = ... }` block currently does everything (save message, load data, assemble context, get model).
2. We need to split it: first block loads data + extracts keys, then we do async Jira fetches (outside the conn lock), then a second small block assembles context and gets model.

**Important:** The `get_jira_credentials` function in `commands.rs` takes a `State<Database>`, but here we need a version that takes `&Database` directly. We need to extract the credential-loading logic. The simplest approach: copy the credential logic inline or make `get_jira_credentials` take `&Database` instead of `State<Database>`.

Actually, looking at the existing code, `get_jira_credentials` in `jira/commands.rs` takes `&Database` (not `State<Database>`). It's a private helper. Let's make it `pub(crate)` so `streaming.rs` can use it.

- [ ] **Step 3: Make `get_jira_credentials` pub(crate)**

In `src-tauri/src/jira/commands.rs`, change:

```rust
fn get_jira_credentials(db: &Database) -> Result<(String, JiraAuth), String> {
```

to:

```rust
pub(crate) fn get_jira_credentials(db: &Database) -> Result<(String, JiraAuth), String> {
```

- [ ] **Step 4: Rewrite `send_message` with Jira injection**

Replace the entire `send_message` function in `src-tauri/src/llm/streaming.rs` with:

```rust
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
```

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: Compiles. There may be warnings about unused imports — that's fine.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/streaming.rs src-tauri/src/jira/commands.rs
git commit -m "feat: inject referenced Jira tickets into LLM context"
```

---

### Task 5: Add `JiraLinkedText` component + `jiraBaseUrlAtom`

**Files:**
- Create: `src/components/JiraLinkedText.tsx`
- Modify: `src/features/settings/settings.atoms.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Add `jiraBaseUrlAtom`**

In `src/features/settings/settings.atoms.ts`, add:

```ts
export const jiraBaseUrlAtom = atom<string | null>(null);
```

- [ ] **Step 2: Load Jira base URL on app start**

In `src/App.tsx`, add the import for the atom and update the existing useEffect:

Add to imports:

```tsx
import { jiraBaseUrlAtom } from "@/features/settings/settings.atoms";
```

Add inside the component, after the existing `setSelectedModel` line:

```tsx
const setJiraBaseUrl = useSetAtom(jiraBaseUrlAtom);
```

In the existing `useEffect`, add after the `get_setting` call for `llm.model`:

```tsx
    invoke<{ base_url: string; auth_method: string; email: string | null } | null>("get_jira_config").then((config) => {
      if (config) setJiraBaseUrl(config.base_url);
    });
```

- [ ] **Step 3: Create `JiraLinkedText` component**

Create `src/components/JiraLinkedText.tsx`:

```tsx
import { useAtomValue } from "jotai";
import { openUrl } from "@tauri-apps/plugin-opener";
import { jiraBaseUrlAtom } from "@/features/settings/settings.atoms";

const TICKET_PATTERN = /([A-Z][A-Z0-9]+-\d+)/g;

interface JiraLinkedTextProps {
  children: string;
}

export function JiraLinkedText({ children }: JiraLinkedTextProps) {
  const jiraBaseUrl = useAtomValue(jiraBaseUrlAtom);

  if (!jiraBaseUrl) {
    return <>{children}</>;
  }

  const parts = children.split(TICKET_PATTERN);
  if (parts.length === 1) {
    return <>{children}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        TICKET_PATTERN.test(part) ? (
          <button
            key={i}
            onClick={() => openUrl(`${jiraBaseUrl}/browse/${part}`)}
            className="text-blue-400 hover:text-blue-300 hover:underline cursor-pointer"
          >
            {part}
          </button>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}
```

Note: `TICKET_PATTERN` has the global flag. After `split`, the matched groups are interleaved with the non-matched parts. We use `test` to check if a part is a ticket ID. Since `test` with a global regex advances `lastIndex`, we need to reset it. Simpler approach — use a non-global regex for the test:

```tsx
import { useAtomValue } from "jotai";
import { openUrl } from "@tauri-apps/plugin-opener";
import { jiraBaseUrlAtom } from "@/features/settings/settings.atoms";

const TICKET_SPLIT = /([A-Z][A-Z0-9]+-\d+)/g;
const TICKET_TEST = /^[A-Z][A-Z0-9]+-\d+$/;

interface JiraLinkedTextProps {
  children: string;
}

export function JiraLinkedText({ children }: JiraLinkedTextProps) {
  const jiraBaseUrl = useAtomValue(jiraBaseUrlAtom);

  if (!jiraBaseUrl) {
    return <>{children}</>;
  }

  const parts = children.split(TICKET_SPLIT);
  if (parts.length === 1) {
    return <>{children}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        TICKET_TEST.test(part) ? (
          <button
            key={i}
            onClick={() => openUrl(`${jiraBaseUrl}/browse/${part}`)}
            className="text-blue-400 hover:text-blue-300 hover:underline cursor-pointer"
          >
            {part}
          </button>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}
```

- [ ] **Step 4: Verify it compiles**

Run: `bun run build 2>&1 | tail -5`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/components/JiraLinkedText.tsx src/features/settings/settings.atoms.ts src/App.tsx
git commit -m "feat: add JiraLinkedText component and jiraBaseUrlAtom"
```

---

### Task 6: Wire `JiraLinkedText` into DumpView and ChatPanel markdown

**Files:**
- Modify: `src/features/session/DumpView.tsx`
- Modify: `src/features/chat/ChatPanel.tsx`

Both files use `<Markdown remarkPlugins={[remarkGfm]}>`. We add a `components` prop that uses `JiraLinkedText` to process text in paragraph and list item elements.

- [ ] **Step 1: Create shared markdown components object**

In both files, we'll use the same pattern. The `react-markdown` `components` prop lets us override how elements are rendered. We override `p` and `li` to process their text children through `JiraLinkedText`.

- [ ] **Step 2: Update DumpView**

In `src/features/session/DumpView.tsx`, add import:

```tsx
import { JiraLinkedText } from "@/components/JiraLinkedText";
```

Find the Markdown usage in the preview section:

```tsx
<Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
```

Replace with:

```tsx
<Markdown
  remarkPlugins={[remarkGfm]}
  components={{
    p: ({ children }) => <p>{processChildren(children)}</p>,
    li: ({ children }) => <li>{processChildren(children)}</li>,
  }}
>{content}</Markdown>
```

Add the `processChildren` helper above the component's return statement (or at the top of the file as a utility):

```tsx
function processChildren(children: React.ReactNode): React.ReactNode {
  return Array.isArray(children)
    ? children.map((child, i) =>
        typeof child === "string" ? <JiraLinkedText key={i}>{child}</JiraLinkedText> : child,
      )
    : typeof children === "string"
      ? <JiraLinkedText>{children}</JiraLinkedText>
      : children;
}
```

Add the React import if not already present:

```tsx
import type React from "react";
```

- [ ] **Step 3: Update ChatPanel**

In `src/features/chat/ChatPanel.tsx`, add import:

```tsx
import { JiraLinkedText } from "@/components/JiraLinkedText";
```

Find the Markdown usage for assistant messages:

```tsx
<Markdown remarkPlugins={[remarkGfm]}>{msg.content}</Markdown>
```

Replace with:

```tsx
<Markdown
  remarkPlugins={[remarkGfm]}
  components={{
    p: ({ children }) => <p>{processChildren(children)}</p>,
    li: ({ children }) => <li>{processChildren(children)}</li>,
  }}
>{msg.content}</Markdown>
```

Add the same `processChildren` helper and React type import as in DumpView.

- [ ] **Step 4: Verify it compiles**

Run: `bun run build 2>&1 | tail -5`
Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/features/session/DumpView.tsx src/features/chat/ChatPanel.tsx
git commit -m "feat: make Jira ticket IDs clickable links in markdown preview"
```

---

### Task 7: Integration test

**Files:** None (testing only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests pass (55 existing + 2 extract + 2 get_issue + 2 context = 61 total, but some existing counts may vary — key is 0 failures).

- [ ] **Step 2: Run frontend build check**

Run: `bun run build 2>&1 | tail -5`
Expected: Builds with no errors.
