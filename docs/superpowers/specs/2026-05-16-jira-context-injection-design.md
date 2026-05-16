# Jira Ticket Context Injection — Design Spec

## Overview

Detect Jira ticket ID patterns (e.g. `FRONT-42`) in notes and chat messages, fetch ticket details from Jira, and inject them into the LLM context so the duck understands what those tickets are about. Also make ticket IDs clickable links in the markdown preview.

## Backend

### New JiraClient method: `get_issue`

```rust
pub async fn get_issue(&self, issue_key: &str) -> AppResult<JiraIssueContext>
```

- Calls `GET /rest/api/2/issue/{key}?fields=summary,status,issuetype,priority,description`
- Returns a `JiraIssueContext` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueContext {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub priority: String,
    pub description: String, // truncated to 500 chars
}
```

- Parses the Jira REST response: `fields.summary`, `fields.status.name`, `fields.issuetype.name`, `fields.priority.name`, `fields.description` (nullable, default to empty string)
- Truncates description to 500 chars (using `chars().take(500)` for UTF-8 safety)
- Uses the same `apply_auth` and `parse_jira_error` patterns as other methods
- Errors on 404 (ticket not found) but callers will handle this gracefully

### New function: `extract_jira_keys`

In `src-tauri/src/jira/client.rs` (or a new `src-tauri/src/jira/extract.rs` if cleaner):

```rust
pub fn extract_jira_keys(text: &str) -> Vec<String>
```

- Regex: `[A-Z][A-Z0-9]+-\d+`
- Deduplicates (preserves first occurrence order)
- Returns unique keys found in the text

### New Tauri command: `fetch_jira_issues`

```rust
#[tauri::command]
pub async fn fetch_jira_issues(db: State<'_, Database>, keys: Vec<String>) -> Result<Vec<JiraIssueContext>, String>
```

- Takes a list of Jira keys
- Fetches each via `get_issue`, skipping any that fail (log warning, don't error)
- Returns the successfully fetched issues
- If Jira is not configured, returns empty vec (no error)

### Modified: `send_message` in `streaming.rs`

The `send_message` function currently:
1. Saves user message
2. Loads notes, tickets, conversation
3. Assembles context
4. Streams LLM response

**New steps inserted between 2 and 3:**

2.5. Extract Jira keys from `note_content` + `content` (the user's message)
2.6. If Jira is configured and keys were found, fetch issues (skip failures)
2.7. Pass fetched issues to `assemble_context`

### Modified: `assemble_context` in `context.rs`

Add a new parameter for Jira issue context:

```rust
pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    jira_issues: &[JiraIssueContext],  // NEW
    conversation: &[(String, String)],
) -> Vec<ChatMessage>
```

If `jira_issues` is non-empty, append a `## Referenced Jira Tickets` section to the system prompt:

```
## Referenced Jira Tickets
- FRONT-42 (Bug, In Progress, High): Fix login timeout — When users attempt to log in with SSO, the request times out after 30s...
- FRONT-55 (Story, To Do, Medium): Add retry logic to auth flow — As a user, I want the login...
```

Format: `- {key} ({type}, {status}, {priority}): {summary} — {description_preview}`

### Caching

In-memory cache in `send_message` is not practical since each invocation is a fresh async call. Instead:

- No caching in v1. Jira API calls are fast (<100ms each) and we're fetching at most a handful of tickets per send.
- If this becomes a bottleneck during dogfooding, add a session-scoped cache later.

### Error handling

- If Jira is not configured: skip extraction entirely, no error
- If individual ticket fetch fails (404, auth error): skip that ticket, log a warning, continue with the rest
- If all fetches fail: proceed with empty Jira context (the chat still works, just without ticket details)
- Network errors don't block the chat flow

## Frontend

### Clickable ticket IDs in markdown preview

In the markdown preview (react-markdown in `DumpView.tsx` and `ChatPanel.tsx`), add a custom component that detects `PROJ-123` patterns in text nodes and wraps them in clickable links.

**Implementation:** A custom `react-markdown` component for `p`, `li`, and `text` nodes that:
1. Splits text on the `[A-Z][A-Z0-9]+-\d+` regex
2. Wraps matches in `<a>` tags that call `openUrl("{base_url}/browse/{key}")`
3. The Jira base URL is read from settings (via `get_jira_config`)

**Shared component:** Create a `JiraLinkedText` component that both DumpView's preview and ChatPanel's assistant messages can use. It takes a string and returns fragments with ticket IDs linked.

**Base URL source:** On app load, fetch `get_jira_config` and store the base URL in a Jotai atom (`jiraBaseUrlAtom`). If not configured, ticket IDs render as plain text (no links).

### New atom

```ts
export const jiraBaseUrlAtom = atom<string | null>(null);
```

Set on app load in `App.tsx` from `get_jira_config().base_url`.

## Out of Scope

- Live detection / inline preview in the CodeMirror editor
- Ticket ID autocomplete
- Caching fetched issues across sends
- Fetching comments or other Jira fields
- Bi-directional sync (updating Jira tickets from rubber-duck)
