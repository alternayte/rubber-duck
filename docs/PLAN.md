# rubber-duck Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

> **Ordering principle:** Phases are ordered by user value for dogfooding, not by implementation convenience. The goal is to use this tool daily as fast as possible.

## Phase 1 — MVP: Dump + Chat + Grill + Tickets

The core loop: brain dump → chat with LLM → get grilled → extract tickets. This is the minimum for daily use.

### Task 1.1 — Scaffold Tauri v2 project
- [x] Initialize Tauri v2 project with React + TypeScript + Vite frontend
- [x] Set up Bun, Tailwind CSS v4, basic layout shell
- [x] Configure Cargo.toml with core dependencies (rusqlite, serde, uuid, chrono, reqwest, tokio, thiserror, tracing)
- [x] Create module structure: `session/`, `ticket/`, `llm/`, `db.rs`, `error.rs`
- [x] Verify `bun tauri dev` launches with skeleton layout

### Task 1.2 — SQLite database + migrations
- [x] Create `db.rs` with `Mutex<Connection>` setup (single-file SQLite)
- [x] Write migration system (versioned SQL files via `include_str!`, forward-only)
- [x] Create tables: sessions, notes, tickets, conversations (Phase 1 only)
- [x] Add FTS5 virtual table for full-text search (`search_index`)
- [x] Write tests for migration runner (9 tests passing)

> **Known limitation:** `Mutex<Connection>` serializes all DB access. Fine for Phase 1 but will need attention when streaming LLM + auto-save + FTS indexing compete for the lock (likely Phase 4). Options: connection pool (`r2d2`), read/write separation, or `tokio::sync::RwLock`.

### Task 1.3 — Session CRUD
- [x] Define `Session` model with serde derives (`session/model.rs`)
- [x] Implement `store.rs`: create, get, list, update, archive, delete (7 tests)
- [x] Implement Tauri commands: `create_session`, `get_session`, `list_sessions`, `update_session`, `archive_session`
- [x] Write tests for store layer (7 tests passing)
- [x] Frontend: session list sidebar + inline create flow (Jotai atoms + shadcn/ui components)

### Task 1.4 — Notes (brain dump) editor
- [x] Define `Note` model (in `session/model.rs`)
- [x] Implement `note_store.rs`: get_or_create, update_content (single note per session, 4 tests)
- [x] Implement Tauri commands: `get_or_create_note`, `update_note`
- [x] Frontend: CodeMirror 6 markdown editor in Dump tab with debounced auto-save (500ms)
- [x] Edit/Preview toggle with react-markdown + remark-gfm rendering
- [x] Bug fix: `update_content` now touches `sessions.updated_at` so edited sessions sort to top

### Task 1.5 — LLM integration + settings
- [x] OpenRouter client with SSE streaming (`llm/client.rs`) — no provider trait, direct OpenRouter API
- [x] Context assembly: system prompt + notes + tickets + last 40 messages (`llm/context.rs`, 6 tests)
- [x] SSE → Tauri event bridge (`llm/streaming.rs`): `llm:chunk`, `llm:done`, `llm:error`
- [x] Settings infrastructure: SQLite `settings` table + OS keychain via `keyring` crate (`settings/`, 5 tests)
- [x] Settings UI: dialog with API key input + model selector (shadcn Dialog/Select + Jotai)
- [x] Curated model list (8 models, default DeepSeek v4 Flash via `llm/models.rs`)
- [x] `send_message` command: saves to DB → assembles context → streams response → saves result

### Task 1.6 — Duck chat panel + Grill mode
- [x] Frontend: ChatPanel component with message list, streaming display, auto-scroll
- [x] Tauri event listeners: `llm:chunk`, `llm:done`, `llm:error` with error classification
- [x] Conversation history: `get_conversation` command, loads on session switch
- [x] Grill mode: `ChatMode` enum, alternate system prompt, Assist/Grill toggle (7 context tests)
- [x] No-API-key state: prompts user to open settings
- [x] Error handling: inline error bubbles for API failures, rate limits, auth errors

> **Why grill mode is here:** It's the differentiating feature. It's a system prompt swap + the toggle already exists. There's no reason to defer it to a later phase.

### Task 1.7a — Ticket model + CRUD
- [x] Define `Ticket`, `CreateTicketParams`, `UpdateTicketParams` models (`ticket/model.rs`)
- [x] Implement `ticket/store.rs`: create, get, list_by_session, update, delete, reorder, set_parent (7 tests)
- [x] Implement 7 Tauri commands: create/get/list/update/delete/reorder/set_parent
- [x] JSON field handling: labels and dependencies as `Vec<String>` ↔ JSON TEXT
- [x] Cascade delete verified via session deletion test

### Task 1.7b — Ticket list UI
- [x] Frontend: collapsible ticket list below editor in Dump tab (title, type, priority, estimate badges)
- [x] Inline editing: click title to edit, click badges to cycle values, expandable description/AC
- [x] Delete with window.confirm, "+ Add" button for manual ticket creation
- [x] Up/down reorder buttons (visible on hover)

### Task 1.7c — LLM ticket extraction
- [x] "Extract Tickets" button in Dump tab header sends structured prompt to LLM
- [x] Parser: extracts ```json code blocks, falls back to raw JSON, handles arrays + single objects
- [x] Field normalization: validates title required, coerces priority/type/estimate against allowlists, defaults for missing fields
- [x] Creates parsed tickets via `create_ticket`, ticket list refreshes automatically
- [x] Error handling: parsing failures shown inline, LLM errors caught via event listener

### Task 1.8 — Basic image paste support
- [x] CodeMirror paste handler intercepts image clipboard data, reads as base64
- [x] `save_pasted_image` Tauri command: base64 decode → save to `{app_data}/images/{session_id}/{uuid}.png`
- [x] Custom `rdimg://` URI scheme protocol serves local images to webview
- [x] Insert `![pasted image](rdimg://localhost/path)` at cursor via `convertFileSrc`
- [x] Images render inline in markdown preview via react-markdown

> **Scope:** Just clipboard paste → save → show inline. No drag-drop, no thumbnails, no LLM vision, no attachment panel. Those stay in Phase 7.

### Task 1.9 — Adversarial review hardening
- [x] **Critical:** Fixed `rdimg://` path traversal — validates paths within `{app_data}/images/` via `canonicalize`
- [x] **Critical:** Fixed silent data loss on conversation save — logs errors via `tracing::error!`, emits `llm:error`
- [x] Created `session/conversation_store.rs` — centralized conversation persistence (2 tests)
- [x] Enabled Content-Security-Policy in `tauri.conf.json`
- [x] `rdimg://` handler detects content-type from file extension, no more hardcoded `image/png`
- [x] Removed `unwrap()` from non-test code in URI scheme handler
- [x] UTF-8 safe ticket description truncation (`chars().take(100)` instead of byte slicing)
- [x] HTTP timeout (120s) on OpenRouter client
- [x] Suppressed dead code warnings (`#[cfg(test)]` for `open_in_memory`, `#[allow(dead_code)]` for unused store functions)
- [x] Wrapped `<App />` in Jotai `<Provider>`
- [x] Fixed extract tickets race condition via shared `isExtractingAtom`
- [x] Ticket textarea saves debounced to `onBlur` instead of every keystroke

> **Test count after hardening:** 42 tests (39 original + 2 conversation_store + 1 UTF-8 truncation)

**Milestone: Full core loop working. Brain dump (with images) → chat → grill → tickets.**

---

## Phase 2 — Slim Jira Push

Minimal Jira integration — enough to push tickets, not a full sync platform.

### Task 2.1 — Jira client
- [x] Implement `JiraClient` with reqwest, base URL from settings
- [x] `JiraAuth` enum: Basic Auth (Cloud: email + API token) and PAT (Server/DC: Bearer token)
- [x] `test_connection` — GET /rest/api/2/myself (v2 to avoid ADF requirement)
- [x] `create_issue` — POST /rest/api/2/issue (summary + description only, plain text via API v2)
- [x] Store `ExternalRef` (Jira issue key + URL) as JSON in `tickets.external_ref` column
- [x] Error handling: `parse_jira_error` extracts Jira structured errors, falls back to status-code messages (401/403/404)
- [x] `JiraUser` handles both Cloud (`accountId`) and Server/DC (`name`) response formats
- [x] 9 tests with mockito mock server (2 store + 3 test_connection + 1 PAT auth + 3 create_issue)
- [x] Tauri commands: `get_jira_config`, `set_jira_config`, `set_jira_api_token`, `has_jira_config`, `test_jira_connection`, `push_ticket_to_jira`
- [x] `auth_method` setting ("basic" or "pat") with backward-compatible default to "basic"

> **Test count after Task 2.1:** 51 tests (44 pre-existing + 2 ExternalRef store + 3 test_connection + 1 PAT + 3 create_issue)

### Task 2.2 — Push UI
- [ ] Jira connection settings in the settings dialog (base URL, email, API token in keychain)
- [ ] "Push to Jira" button on individual tickets
- [ ] Project picker (fetch from Jira API)
- [ ] Show pushed status with link to Jira issue
- [ ] Error states: connection failed, auth invalid, push rejected — clear user-facing messages

> **Deliberately slim:** No ADF conversion, no field mapping UI, no batch push, no epic linking to Jira. Those come after dogfooding reveals what's actually needed.

**Milestone: Can push tickets to Jira. The full dogfooding loop is complete.**

---

## 🔄 Dogfood Checkpoint

**Stop building features. Use the tool for 2 weeks of real planning work at Frontiers.**

During this period:
- Track what's painful, what's missing, what's unnecessary
- Note which Phase 3+ features you actually want vs. which seemed important in theory
- Reprioritize the remaining phases based on real usage
- File bugs and UX issues as sessions in rubber-duck itself

The phases below are the current best-guess ordering. **Expect this to change after dogfooding.**

---

## Phase 3 — Refine View + Board View

### Task 3.1 — Refine view (split pane)
- [ ] Frontend: "Refine" tab showing notes on the left (read-only), tickets on the right (editable)
- [ ] Tickets rendered as expandable cards with all fields editable

### Task 3.2 — Board view (kanban)
- [ ] Frontend: "Board" tab with kanban columns by status (Draft → Refined → Ready to Push → Pushed)
- [ ] Drag-and-drop reorder within and across columns
- [ ] Epic grouping: drag a ticket onto another to create parent-child relationship

### Task 3.3 — Ticket refinement via LLM
- [ ] Per-ticket LLM actions: "improve description", "write acceptance criteria", "split this ticket", "estimate"
- [ ] LLM response updates the ticket in-place (with undo)

---

## Phase 4 — Session Memory (FTS)

> **FTS indexing pipeline:** The `search_index` FTS5 table exists but is unpopulated. This phase must implement the indexing pipeline: populate the FTS table from note saves, ticket creates/updates, and conversation messages. Options: SQLite triggers, or explicit indexing in the Rust store functions.

> **Mutex scaling note:** With FTS indexing + search + streaming LLM + auto-save all competing for the DB lock, the single `Mutex<Connection>` may become a bottleneck. Evaluate if `r2d2` connection pool or read/write lock separation is needed.

### Task 4.1 — FTS indexing pipeline
- [ ] Populate `search_index` from existing data (backfill)
- [ ] Index on write: update FTS on note save, ticket create/update, conversation save
- [ ] Verify search works: insert data, query, check results

### Task 4.2 — Auto-summarize on archive
- [ ] When a session is archived, call LLM to generate summary + key decisions
- [ ] Store as `SessionMemory` record
- [ ] Show summary on archived session card

### Task 4.3 — Cross-session search
- [ ] Tauri command: `search_global(query)` → ranked results with session context
- [ ] Frontend: cmd+K search modal — results grouped by session
- [ ] Clicking a result opens that session

### Task 4.4 — Automatic context injection
- [ ] When sending a chat message, search memory for relevant past context
- [ ] Include top-N relevant snippets in LLM context
- [ ] Show indicator: "Drawing on N past sessions"

---

## Phase 5 — Repo Context

> Moved before Document Generation because doc templates reference repo context. Without repo context, the SDD template is useless.

### Task 5.1 — Repo attachment
- [ ] `RepoContext` model and store
- [ ] Tauri command: `attach_repo(session_id, source)` — local path or git URL
- [ ] For local paths: validate and store path reference

### Task 5.2 — Repo indexing
- [ ] Generate repo summary: directory tree, key files, file count by extension
- [ ] Store summary in RepoContext record
- [ ] Support re-indexing on demand

### Task 5.3 — LLM context integration
- [ ] Include repo summary in LLM context when available
- [ ] Allow user to "focus" on specific files/directories
- [ ] LLM can reference actual file paths in ticket descriptions

---

## Phase 6 — Document Generation

### Task 6.1 — Template system
- [ ] Built-in templates: PRD, SDD, Test Plan, ADR (markdown with LLM directives)
- [ ] Templates gracefully degrade without repo context (omit repo-dependent sections or prompt user)
- [ ] Tauri commands: list/get/create/update templates

### Task 6.2 — Document generator
- [ ] Parse template, call LLM per section with session context
- [ ] Assemble into complete document with version tracking
- [ ] Support regenerating individual sections

### Task 6.3 — Document UI
- [ ] Generate menu, template picker, doc viewer/editor
- [ ] Export: copy as markdown, save as .md file
- [ ] Version history: see and restore previous generations

---

## Phase 7 — Image + File Attachments

### Task 7.1 — Image paste and drag-drop
- [ ] Handle clipboard paste and file drag-drop
- [ ] Store in app data directory, reference in SQLite
- [ ] Display inline in markdown editor

### Task 7.2 — LLM vision support
- [ ] Include images in LLM context for vision-capable models
- [ ] "Describe this image" and "reference this diagram" actions

---

## Phase 8 — Vector RAG Layer

### Task 8.1 — Embedding pipeline
- [ ] Local embeddings via fastembed-rs or API-based
- [ ] Chunk and embed session content
- [ ] Store vectors using sqlite-vec

### Task 8.2 — Semantic search
- [ ] Hybrid search: vector similarity + FTS5 exact matches
- [ ] Augment memory context injection with RAG results

---

## Phase 9 — Excalidraw Integration

### Task 9.1 — Excalidraw viewer
- [ ] Embed Excalidraw React component
- [ ] Import .excalidraw files, render inline

### Task 9.2 — LLM diagram generation
- [ ] LLM outputs Excalidraw JSON from descriptions
- [ ] Diagrams editable and stored as attachments

---

## Phase 10 — Push to Linear

### Task 10.1 — Linear implementation
- [ ] GraphQL client, API key auth
- [ ] Map ticket fields to Linear model (markdown description — simpler than Jira ADF)
- [ ] Push UI: reuse Phase 2 UI with Linear-specific fields

---

## Stretch Goals (Unscheduled)

- [ ] Full Jira ADF conversion for rich descriptions
- [ ] Jira field mapping UI, batch push, epic linking
- [ ] Bidirectional sync — pull ticket status updates back
- [ ] Offline LLM via Ollama
- [ ] Voice-to-text input (whisper.cpp)
- [ ] Keyboard shortcuts (vim-style)
- [ ] Session templates
- [ ] Export session as shareable markdown
- [ ] PDF export via print-to-PDF
- [ ] Plugin system for additional integrations
- [ ] Grill mode gap tracking (which topics have been reviewed, coverage indicators)
