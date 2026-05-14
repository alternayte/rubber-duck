# Settings + OpenRouter LLM Integration — Design Spec

## Overview

Add a settings infrastructure and OpenRouter-based LLM integration to rubber-duck, enabling the duck chat panel to send messages, stream responses, and persist conversations. This covers Task 1.5 from the implementation plan plus the new settings feature area.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| LLM provider | OpenRouter (no abstraction) | One API, multi-model, eliminates provider trait complexity |
| API key storage | OS keychain via `keyring` crate | Secure, no plaintext secrets in DB or config files |
| Settings storage | SQLite `settings` table | Consistent with rest of app, single source of truth |
| Settings UI | Modal dialog from gear icon in sidebar | Clean separation from main workflow |
| Model list | Curated hardcoded list (~8 models) | Simple, no API call needed, we control UX |
| Default model | DeepSeek v4 Flash | Cheap, fast, large context — good for daily use |
| Context strategy | Full inclusion + 40-message conversation cap | Notes/tickets are user-bounded; conversation is the unbounded risk |
| Context trimming | Deferred to Phase 4 | YAGNI — 40-message cap is sufficient for v1 |

## Settings Infrastructure

### Database Schema

New migration `003_add_settings.sql`:

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    category TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Initial Settings

| Key | Default | Category | Description |
|-----|---------|----------|-------------|
| `llm.model` | `deepseek/deepseek-chat-v4-0324:free` | llm | OpenRouter model ID |
| `llm.api_key_ref` | `""` (empty = not set) | llm | Keychain reference indicator |

### Settings Store (`settings/store.rs`)

Simple key-value operations:
- `get(conn, key) -> Option<String>`
- `set(conn, key, value, category)`
- `get_by_category(conn, category) -> Vec<(key, value)>`

### API Key Flow

1. User opens settings dialog
2. Enters OpenRouter API key
3. Frontend calls `set_api_key(key)` Tauri command
4. Backend stores key in OS keychain (via `keyring` crate)
5. Backend sets `llm.api_key_ref = "openrouter"` in settings table
6. To retrieve: backend reads keychain directly when making API calls

### Settings UI

Gear icon in the left session sidebar footer (below "+ New Session") opens a modal dialog containing:
- **API Key**: masked text input with show/hide toggle, save button
- **Model**: dropdown selector with curated model list
- Visual indicator of whether an API key is currently set (green check / red warning)

### Tauri Commands

- `get_setting(key)` → `Option<String>`
- `set_setting(key, value, category)`
- `set_api_key(key)` → stores in keychain + updates settings
- `has_api_key()` → `bool`
- `get_available_models()` → `Vec<ModelInfo>` (returns curated list)

## LLM Module

### Architecture

No provider trait. Direct OpenRouter client.

```
llm/
  mod.rs          — module declarations
  client.rs       — OpenRouter HTTP client (streaming)
  context.rs      — context assembly from session data
  streaming.rs    — SSE stream → Tauri event bridge
  models.rs       — curated model list with metadata
```

### OpenRouter Client (`llm/client.rs`)

Single function:

```rust
pub async fn stream_completion(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
) -> AppResult<impl Stream<Item = AppResult<StreamEvent>>>
```

- Calls `POST https://openrouter.ai/api/v1/chat/completions`
- Headers: `Authorization: Bearer {api_key}`, `HTTP-Referer: rubber-duck`, `X-Title: rubber-duck`
- Body: `{ model, messages, stream: true }`
- Returns a stream of parsed SSE events

### SSE Event Format (OpenRouter/OpenAI compatible)

```
data: {"id":"...","choices":[{"delta":{"content":"Hello"},"index":0}]}
data: {"id":"...","choices":[{"delta":{},"finish_reason":"stop","index":0}]}
data: [DONE]
```

### Stream Events (`llm/streaming.rs`)

Bridges SSE → Tauri events:

| Tauri Event | Payload | When |
|-------------|---------|------|
| `llm:chunk` | `{ content: String }` | Each content delta |
| `llm:done` | `{ full_content: String }` | Stream complete |
| `llm:error` | `{ message: String }` | Error at any point |

### Context Assembly (`llm/context.rs`)

Assembles the messages array sent to OpenRouter:

```rust
pub fn assemble_context(
    session: &Session,
    note_content: &str,
    tickets: &[Ticket],
    conversation: &[Conversation],  // last 40 messages
) -> Vec<ChatMessage>
```

**Message order:**
1. **System message**: assist-mode persona + session context field + note content + ticket summaries
2. **Conversation messages**: last 40 messages mapped to `{ role, content }`

Tickets are formatted as a structured text block within the system message:

```
## Current Tickets
- [T1] Fix login bug (Task, High, Draft) — Fix the authentication...
- [T2] Add dark mode (Story, Medium, Draft) — Implement theme...
```

### Curated Model List (`llm/models.rs`)

```rust
pub struct ModelInfo {
    pub id: &'static str,       // OpenRouter model ID
    pub name: &'static str,     // Display name
    pub context_window: u32,    // Max tokens
}
```

Models:
- DeepSeek v4 Flash (default)
- DeepSeek v4
- Claude Sonnet 4
- Claude Haiku 4
- GPT-4o
- GPT-4o Mini
- Llama 4 Scout
- Qwen 3 235B

### Request Flow

1. User types message in duck chat, presses Enter
2. Frontend calls `send_message(session_id, content)` Tauri command
3. Backend saves user message to `conversations` table
4. Backend loads: session, note content, tickets, last 40 conversation messages
5. Backend assembles context via `context.rs`
6. Backend reads API key from keychain, model from settings
7. Backend calls `stream_completion()` → spawns async task
8. Async task reads SSE stream, emits Tauri events per chunk
9. Frontend listens to `llm:chunk` events, appends to response display
10. On `llm:done`, backend saves full assistant message to `conversations`
11. On `llm:error`, error shown in chat as a system message

### Error States

| Condition | Behavior |
|-----------|----------|
| No API key set | Chat panel shows "Set up your OpenRouter API key in Settings to start chatting" with a link/button to open settings |
| Invalid API key | `llm:error` event with "Invalid API key" message |
| Network error | `llm:error` event with connection error details |
| Stream interrupted | Partial response saved to DB, error bubble shown after partial content |
| Rate limited | `llm:error` with retry-after hint if available |

## New Tauri Module: Settings

Following the existing pattern (package-by-feature):

```
settings/
  mod.rs
  model.rs       — no model struct needed, just key-value
  store.rs       — get/set/get_by_category
  commands.rs    — Tauri commands for settings + API key
```

## Frontend Changes

### Settings Dialog Component

New component: `src/features/settings/SettingsDialog.tsx`
- Triggered by gear icon in sidebar
- Uses shadcn Dialog, Input, Select components
- Manages API key input (masked) and model selection
- Shows save confirmation

### Jotai Atoms

- `apiKeySetAtom` — boolean, whether API key is configured
- `selectedModelAtom` — current model ID from settings

### Duck Chat Updates (Task 1.6 scope, but designed now)

The duck chat panel (currently a placeholder) will need:
- Message input that calls `send_message`
- Streaming response display (listening to Tauri events)
- Error state handling
- "Configure API key" prompt when no key is set

This spec covers the infrastructure (Task 1.5). The chat UI wiring is Task 1.6.

## Out of Scope

- Provider abstraction / multiple provider support
- Token counting or smart context trimming
- Conversation summarization
- Grill mode (separate system prompt — Task 9.1)
- Custom model input (only curated list for now)
