# Jira Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Jira REST API client that can test connections and push tickets from rubber-duck to Jira Cloud.

**Architecture:** A `jira/` feature module containing an HTTP client (`JiraClient`) that talks to Jira Cloud REST API v2, plus Tauri commands that wire it to the frontend. Jira credentials (base URL, email) live in the existing settings table; the API token goes in the OS keychain. Pushed tickets store an `ExternalRef` JSON blob in the existing `tickets.external_ref` column.

**Tech Stack:** reqwest (HTTP), mockito (test mocks), keyring (credential storage), serde_json (serialization)

---

## File Structure

```
src-tauri/src/jira/
├── mod.rs         — module declarations
├── model.rs       — Jira API request/response types
├── client.rs      — JiraClient: test_connection, create_issue
└── commands.rs    — Tauri commands: test/push/settings
```

**Modified files:**
- `src-tauri/Cargo.toml` — add `mockito` dev dependency
- `src-tauri/src/ticket/model.rs` — add `ExternalRef` struct
- `src-tauri/src/ticket/store.rs` — add `set_external_ref` function
- `src-tauri/src/error.rs` — add `Http` variant for reqwest errors
- `src-tauri/src/lib.rs` — register `jira` module + commands

---

### Task 1: ExternalRef model + ticket store helper

**Files:**
- Modify: `src-tauri/src/ticket/model.rs`
- Modify: `src-tauri/src/ticket/store.rs`

- [ ] **Step 1: Add ExternalRef struct to ticket model**

```rust
// Append to src-tauri/src/ticket/model.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    pub platform: String,
    pub key: String,
    pub url: String,
}
```

- [ ] **Step 2: Write failing test for set_external_ref**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/ticket/store.rs`:

```rust
#[test]
fn set_and_read_external_ref() {
    let db = test_db();
    let conn = db.conn().unwrap();
    let session_id = make_session(&conn);
    let ticket = create(&conn, &minimal_params(&session_id, "Push me")).unwrap();

    assert!(ticket.external_ref.is_none());

    let ext_ref = serde_json::to_string(&super::super::model::ExternalRef {
        platform: "jira".to_string(),
        key: "PROJ-123".to_string(),
        url: "https://site.atlassian.net/browse/PROJ-123".to_string(),
    })
    .unwrap();

    set_external_ref(&conn, &ticket.id, Some(&ext_ref)).unwrap();
    let updated = get(&conn, &ticket.id).unwrap();
    assert_eq!(updated.external_ref, Some(ext_ref));
}

#[test]
fn clear_external_ref() {
    let db = test_db();
    let conn = db.conn().unwrap();
    let session_id = make_session(&conn);
    let ticket = create(&conn, &minimal_params(&session_id, "Push me")).unwrap();

    let ext_ref = r#"{"platform":"jira","key":"PROJ-1","url":"https://x.atlassian.net/browse/PROJ-1"}"#;
    set_external_ref(&conn, &ticket.id, Some(ext_ref)).unwrap();

    set_external_ref(&conn, &ticket.id, None).unwrap();
    let cleared = get(&conn, &ticket.id).unwrap();
    assert!(cleared.external_ref.is_none());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test ticket::store::tests::set_and_read_external_ref ticket::store::tests::clear_external_ref -- --nocapture 2>&1`
Expected: compilation error — `set_external_ref` not defined

- [ ] **Step 4: Implement set_external_ref**

Add to `src-tauri/src/ticket/store.rs` (after the `set_parent` function):

```rust
pub fn set_external_ref(conn: &Connection, id: &str, external_ref: Option<&str>) -> AppResult<Ticket> {
    conn.execute(
        "UPDATE tickets SET external_ref = ?1 WHERE id = ?2",
        params![external_ref, id],
    )?;
    get(conn, id)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test ticket::store::tests::set_and_read_external_ref ticket::store::tests::clear_external_ref -- --nocapture 2>&1`
Expected: both PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/ticket/model.rs src-tauri/src/ticket/store.rs
git commit -m "feat: add ExternalRef model and set_external_ref store function"
```

---

### Task 2: Jira module scaffold + dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/error.rs`
- Create: `src-tauri/src/jira/mod.rs`
- Create: `src-tauri/src/jira/model.rs`
- Create: `src-tauri/src/jira/client.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add mockito dev dependency**

Add to `src-tauri/Cargo.toml` at the end of the file:

```toml
[dev-dependencies]
mockito = "1"
```

- [ ] **Step 2: Add Http error variant**

Replace the `AppError` enum in `src-tauri/src/error.rs`:

```rust
#[derive(Error, Debug)]
pub enum AppError {
    #[error("{0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Other(String),
    #[error("Keyring error: {0}")]
    Keyring(String),
}
```

- [ ] **Step 3: Create jira/model.rs with API types**

```rust
// src-tauri/src/jira/model.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CreateIssueRequest {
    pub fields: CreateIssueFields,
}

#[derive(Debug, Serialize)]
pub struct CreateIssueFields {
    pub project: ProjectRef,
    pub summary: String,
    pub description: String,
    pub issuetype: IssueTypeRef,
}

#[derive(Debug, Serialize)]
pub struct ProjectRef {
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct IssueTypeRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraUser {
    pub account_id: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueResponse {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraErrorResponse {
    #[serde(default)]
    pub error_messages: Vec<String>,
    #[serde(default)]
    pub errors: std::collections::HashMap<String, String>,
}
```

- [ ] **Step 4: Create jira/client.rs skeleton**

```rust
// src-tauri/src/jira/client.rs
use reqwest::Client;
use std::time::Duration;

use crate::error::AppResult;

pub struct JiraClient {
    client: Client,
    base_url: String,
    email: String,
    api_token: String,
}

impl JiraClient {
    pub fn new(base_url: &str, email: &str, api_token: &str) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            email: email.to_string(),
            api_token: api_token.to_string(),
        })
    }
}
```

- [ ] **Step 5: Create jira/mod.rs**

```rust
// src-tauri/src/jira/mod.rs
pub mod client;
pub mod model;
```

- [ ] **Step 6: Register jira module in lib.rs**

Add `mod jira;` to the module declarations in `src-tauri/src/lib.rs` (after `mod ticket;`):

```rust
mod jira;
```

- [ ] **Step 7: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: compiles with no errors (warnings about unused code are OK)

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/error.rs src-tauri/src/jira/ src-tauri/src/lib.rs
git commit -m "feat: scaffold jira module with API types and client skeleton"
```

---

### Task 3: JiraClient::test_connection

**Files:**
- Modify: `src-tauri/src/jira/client.rs`

- [ ] **Step 1: Write failing test for test_connection success**

Add to `src-tauri/src/jira/client.rs`:

```rust
// At top of file, add imports:
use crate::error::{AppError, AppResult};
use super::model::{JiraUser, JiraErrorResponse};

// After the impl block, add:
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/rest/api/2/myself")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"accountId":"abc123","displayName":"Test User"}"#)
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), "test@example.com", "token").unwrap();
        let user = client.test_connection().await.unwrap();

        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.account_id, "abc123");
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test jira::client::tests::test_connection_success -- --nocapture 2>&1`
Expected: compilation error — `test_connection` method not found

- [ ] **Step 3: Implement test_connection**

Add to the `impl JiraClient` block in `src-tauri/src/jira/client.rs`:

```rust
pub async fn test_connection(&self) -> AppResult<JiraUser> {
    let url = format!("{}/rest/api/2/myself", self.base_url);
    let response = self
        .client
        .get(&url)
        .basic_auth(&self.email, Some(&self.api_token))
        .send()
        .await?;

    if response.status().is_success() {
        let user: JiraUser = response.json().await?;
        return Ok(user);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = parse_jira_error(&body, status.as_u16());
    Err(AppError::Other(message))
}
```

Add helper function outside the `impl` block:

```rust
fn parse_jira_error(body: &str, status: u16) -> String {
    if let Ok(err) = serde_json::from_str::<JiraErrorResponse>(body) {
        let messages: Vec<&str> = err
            .error_messages
            .iter()
            .map(|s| s.as_str())
            .chain(err.errors.values().map(|s| s.as_str()))
            .collect();
        if !messages.is_empty() {
            return messages.join("; ");
        }
    }

    match status {
        401 => "Authentication failed — check your email and API token".to_string(),
        403 => "Permission denied — check your Jira permissions".to_string(),
        404 => "Jira site not found — check your base URL".to_string(),
        _ => format!("Jira API error (HTTP {status})"),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test jira::client::tests::test_connection_success -- --nocapture 2>&1`
Expected: PASS

- [ ] **Step 5: Write and run test for auth failure**

Add to the `tests` module:

```rust
#[tokio::test]
async fn test_connection_auth_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/myself")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"errorMessages":["You do not have the permission"],"errors":{}}"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), "bad@example.com", "wrong").unwrap();
    let result = client.test_connection().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("permission"), "Expected permission error, got: {err}");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_connection_fallback_error_message() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/myself")
        .with_status(401)
        .with_body("not json")
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), "bad@example.com", "wrong").unwrap();
    let result = client.test_connection().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Authentication failed"),
        "Expected auth fallback message, got: {err}"
    );
    mock.assert_async().await;
}
```

Run: `cd src-tauri && cargo test jira::client::tests -- --nocapture 2>&1`
Expected: all 3 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/jira/client.rs
git commit -m "feat: implement JiraClient::test_connection with error handling"
```

---

### Task 4: JiraClient::create_issue

**Files:**
- Modify: `src-tauri/src/jira/client.rs`

- [ ] **Step 1: Write failing test for create_issue success**

Add import at top of `src-tauri/src/jira/client.rs`:

```rust
use super::model::{
    CreateIssueRequest, CreateIssueFields, ProjectRef, IssueTypeRef,
    JiraUser, JiraErrorResponse, CreateIssueResponse,
};
use crate::ticket::model::ExternalRef;
```

(Replace the existing `use super::model::{JiraUser, JiraErrorResponse};` line.)

Add test:

```rust
#[tokio::test]
async fn create_issue_success() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/rest/api/2/issue")
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(r#"{"id":"10001","key":"PROJ-42","self":"https://site.atlassian.net/rest/api/2/issue/10001"}"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), "test@example.com", "token").unwrap();
    let ext_ref = client
        .create_issue("PROJ", "Fix the bug", "It's broken", "Bug")
        .await
        .unwrap();

    assert_eq!(ext_ref.platform, "jira");
    assert_eq!(ext_ref.key, "PROJ-42");
    assert!(ext_ref.url.contains("/browse/PROJ-42"));
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test jira::client::tests::create_issue_success -- --nocapture 2>&1`
Expected: compilation error — `create_issue` method not found

- [ ] **Step 3: Implement create_issue**

Add to the `impl JiraClient` block:

```rust
pub async fn create_issue(
    &self,
    project_key: &str,
    summary: &str,
    description: &str,
    issue_type: &str,
) -> AppResult<ExternalRef> {
    let url = format!("{}/rest/api/2/issue", self.base_url);
    let body = CreateIssueRequest {
        fields: CreateIssueFields {
            project: ProjectRef {
                key: project_key.to_string(),
            },
            summary: summary.to_string(),
            description: description.to_string(),
            issuetype: IssueTypeRef {
                name: issue_type.to_string(),
            },
        },
    };

    let response = self
        .client
        .post(&url)
        .basic_auth(&self.email, Some(&self.api_token))
        .json(&body)
        .send()
        .await?;

    if response.status().is_success() {
        let created: CreateIssueResponse = response.json().await?;
        let browse_url = format!("{}/browse/{}", self.base_url, created.key);
        return Ok(ExternalRef {
            platform: "jira".to_string(),
            key: created.key,
            url: browse_url,
        });
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = parse_jira_error(&body, status.as_u16());
    Err(AppError::Other(message))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test jira::client::tests::create_issue_success -- --nocapture 2>&1`
Expected: PASS

- [ ] **Step 5: Write and run tests for error cases**

Add to the `tests` module:

```rust
#[tokio::test]
async fn create_issue_validation_error() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/rest/api/2/issue")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"errorMessages":[],"errors":{"project":"project is required"}}"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), "test@example.com", "token").unwrap();
    let result = client.create_issue("BAD", "Title", "Desc", "Task").await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("project is required"),
        "Expected field error, got: {err}"
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn create_issue_auth_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/rest/api/2/issue")
        .with_status(401)
        .with_body("")
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), "bad@example.com", "wrong").unwrap();
    let result = client.create_issue("PROJ", "Title", "Desc", "Task").await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Authentication failed"),
        "Expected auth error, got: {err}"
    );
    mock.assert_async().await;
}
```

Run: `cd src-tauri && cargo test jira::client::tests -- --nocapture 2>&1`
Expected: all 6 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/jira/client.rs
git commit -m "feat: implement JiraClient::create_issue with error handling"
```

---

### Task 5: Jira Tauri commands

**Files:**
- Create: `src-tauri/src/jira/commands.rs`
- Modify: `src-tauri/src/jira/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create jira/commands.rs with settings commands**

The Jira API token uses a separate keyring entry from OpenRouter. Settings use the existing key-value store with category `"jira"`.

```rust
// src-tauri/src/jira/commands.rs
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Database;
use crate::error::AppError;
use crate::settings::store as settings_store;

use super::client::JiraClient;
use super::model::JiraUser;

const KEYRING_SERVICE: &str = "rubber-duck";
const JIRA_KEYRING_USER: &str = "jira-api-token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    pub base_url: String,
    pub email: String,
}

fn get_jira_credentials(db: &Database) -> Result<(String, String, String), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let base_url = settings_store::get(&conn, "jira.base_url")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Jira base URL not configured".to_string())?;
    let email = settings_store::get(&conn, "jira.email")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Jira email not configured".to_string())?;

    let entry = keyring::Entry::new(KEYRING_SERVICE, JIRA_KEYRING_USER)
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;
    let api_token = entry
        .get_password()
        .map_err(|e| AppError::Keyring(e.to_string()).to_string())?;

    Ok((base_url, email, api_token))
}

#[tauri::command]
pub fn get_jira_config(db: State<Database>) -> Result<Option<JiraConfig>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let base_url = settings_store::get(&conn, "jira.base_url").map_err(|e| e.to_string())?;
    let email = settings_store::get(&conn, "jira.email").map_err(|e| e.to_string())?;

    match (base_url, email) {
        (Some(base_url), Some(email)) => Ok(Some(JiraConfig { base_url, email })),
        _ => Ok(None),
    }
}

#[tauri::command]
pub fn set_jira_config(db: State<Database>, base_url: String, email: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let normalized = base_url.trim_end_matches('/').to_string();
    settings_store::set(&conn, "jira.base_url", &normalized, "jira").map_err(|e| e.to_string())?;
    settings_store::set(&conn, "jira.email", &email, "jira").map_err(|e| e.to_string())?;
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
    let has_email = settings_store::get(&conn, "jira.email")
        .map_err(|e| e.to_string())?
        .is_some_and(|v| !v.is_empty());
    let has_token = keyring::Entry::new(KEYRING_SERVICE, JIRA_KEYRING_USER)
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some_and(|v| !v.is_empty());
    Ok(has_url && has_email && has_token)
}

#[tauri::command]
pub async fn test_jira_connection(db: State<'_, Database>) -> Result<JiraUser, String> {
    let (base_url, email, api_token) = get_jira_credentials(&db)?;
    let client = JiraClient::new(&base_url, &email, &api_token).map_err(|e| e.to_string())?;
    client.test_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn push_ticket_to_jira(
    db: State<'_, Database>,
    ticket_id: String,
    project_key: String,
) -> Result<crate::ticket::model::Ticket, String> {
    let (base_url, email, api_token) = get_jira_credentials(&db)?;
    let client = JiraClient::new(&base_url, &email, &api_token).map_err(|e| e.to_string())?;

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
```

- [ ] **Step 2: Update jira/mod.rs to include commands**

```rust
// src-tauri/src/jira/mod.rs
pub mod client;
pub mod commands;
pub mod model;
```

- [ ] **Step 3: Register Jira commands in lib.rs**

Add `use jira::commands::*;` to the imports and add the commands to `generate_handler![]`:

Add import:
```rust
use jira::commands::*;
```

Add to the `generate_handler![]` macro (after `save_pasted_image,`):
```rust
get_jira_config,
set_jira_config,
set_jira_api_token,
has_jira_config,
test_jira_connection,
push_ticket_to_jira,
```

- [ ] **Step 4: Make settings store accessible from jira module**

The `settings` module is currently `mod settings;` (private). The `jira/commands.rs` imports `crate::settings::store`. Check if this compiles — the `store` module inside `settings` needs to be `pub`.

Check `src-tauri/src/settings/mod.rs`. If `store` is already `pub mod store;`, no change needed. If not, make it public.

Similarly, `ticket::store` and `ticket::model` need to be accessible from `jira::commands`. Check `src-tauri/src/ticket/mod.rs` — it already has `pub mod store;` and `pub mod model;`.

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: compiles with no errors

- [ ] **Step 6: Run all tests to verify nothing is broken**

Run: `cd src-tauri && cargo test 2>&1`
Expected: all existing tests pass + new tests pass

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/jira/commands.rs src-tauri/src/jira/mod.rs src-tauri/src/lib.rs src-tauri/src/settings/mod.rs
git commit -m "feat: add Jira Tauri commands for settings, test connection, and push"
```
