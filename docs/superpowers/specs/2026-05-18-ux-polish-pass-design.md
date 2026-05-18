# UX Polish Pass — Design Spec

**Date:** 2026-05-18
**Status:** Approved
**Scope:** 8 changes across the full app to address daily-use pain points

---

## 1. Chat Input Overhaul

**Problem:** Chat input is a single-line `<Input>` component. Can't see multi-line messages.

**Design:**
- Replace `<Input>` in `AtMentionInput.tsx` with a `<textarea>`.
- Auto-grow via a `useAutoResize` hook that sets `style.height` to `scrollHeight` on each input event.
- Min height: 1 line (~40px). Max height: 40vh. After max, textarea scrolls internally.
- Enter sends message. Shift+Enter inserts newline.
- Auto-focus on mount and after sending.
- @ mention trigger still works inside textarea (same detection logic, wired to textarea events).
- Disabled with placeholder "Generating..." during streaming.

**Files affected:**
- `src/components/AtMentionInput.tsx` — replace Input with textarea, add auto-resize
- `src/features/chat/ChatPanel.tsx` — update input container styling

---

## 2. Stop Generation

**Problem:** No way to cancel a streaming response. Once generation starts, it runs to completion or times out at 120s.

**Frontend:**
- "Stop" button (square icon) replaces Send button during streaming.
- Click fires `cancel_generation` Tauri command.
- Partial response is kept and saved to conversation history.
- Input re-enables immediately after cancellation.
- Same pattern in `DocsView`: each section being generated gets an inline stop button.

**Backend (Rust):**
- Add `tokio_util::sync::CancellationToken` to the streaming task.
- Store active tokens in app state (`DashMap<String, CancellationToken>`) keyed by conversation_id (after multi-chat migration; session_id initially).
- New command: `cancel_generation(conversation_id)` — looks up token, calls `.cancel()`.
- Streaming loop in `streaming.rs`: check `token.is_cancelled()` on each chunk. If cancelled: emit `llm:cancelled` event, save partial content, remove token from map.
- For doc gen: same pattern per-section in `generator.rs`.

**Edge cases:**
- Double-click stop: idempotent (token already cancelled).
- Stop after completion: no-op (token already dropped).
- Network timeout: existing 120s timeout applies as fallback.

**New dependency (Rust):** `tokio-util` (for `CancellationToken`)

**Files affected:**
- `src-tauri/src/llm/streaming.rs` — add cancellation token to stream loop
- `src-tauri/src/llm/client.rs` — accept token, check on each chunk
- `src-tauri/src/lib.rs` — add cancellation map to app state
- `src-tauri/src/docs/generator.rs` — add per-section cancellation
- `src/features/chat/ChatPanel.tsx` — add Stop button, wire `cancel_generation` command
- `src/features/docs/DocumentCard.tsx` — add inline stop button during section generation

---

## 3. Markdown Rendering + Message Actions

**Problem:** LLM responses are unstyled. No syntax highlighting for code blocks. No way to copy messages or code.

**Code blocks:**
- Use `react-syntax-highlighter` with `oneDark` theme (matches existing CodeMirror theme).
- Custom `code` component in react-markdown: detects fenced blocks (has `className` with language), renders with highlighter.
- Language label (top-left) parsed from info string.
- Copy button (top-right) copies code content to clipboard. Shows "Copied!" for 2s.
- Inline code: `bg-muted rounded px-1 py-0.5 text-sm font-mono`.

**Message actions:**
- Copy button on each message, visible on hover (top-right of message).
- Copies entire message content as raw markdown.
- All text is user-selectable (ensure no `select-none` or `pointer-events-none` on message content).

**Prose styling:** Already using `prose prose-invert prose-sm` from `@tailwindcss/typography`. This handles bold, italic, headers, lists, links, blockquotes.

**New dependency (npm):** `react-syntax-highlighter`

**Files affected:**
- `src/features/chat/ChatPanel.tsx` — add custom code component to Markdown, add message-level copy button
- `src/features/session/DumpView.tsx` — same code component for consistency

---

## 4. @ Mention Rework

**Problem:** Dropdown gets clipped by container, positioned wrong, file paths hard to read.

**Design:**
- Render dropdown as a React portal (document root) to avoid clipping.
- Position anchored to textarea caret using `textarea-caret` lib (or manual calculation via a hidden mirror div).
- When `@` is typed, dropdown appears near the caret position.

**Result display:**
- Directory path (dimmed text) + filename (bright) + file type icon/badge.
- Repo name as section header when multiple repos attached.
- Max 8 visible results, scrollable beyond that.
- Arrow keys navigate, Enter selects, Escape closes.
- Fuzzy matching on the client side: simple case-insensitive substring matching on file path for v1 (filter results from `search_repo_files`).

**After selection:** Insert `@reponame/path/to/file` at cursor position (existing format, backend already parses it).

**New dependency (npm):** `textarea-caret` (or equivalent — tiny lib for getting pixel coordinates of a textarea caret)

**Files affected:**
- `src/components/AtMentionInput.tsx` — rewrite dropdown as portal, add caret positioning, fuzzy filter
- `src/components/MentionText.tsx` — no changes needed (display component is fine)

---

## 5. Ticket Link Bug Fix

**Problem:** Ticket keys in chat used to hyperlink to Jira but stopped working. Settings are correct, test endpoint works.

**Investigation approach:**
1. Trace the prop chain: settings atom → ChatPanel → JiraLinkedText. Verify `jiraBaseUrl` reaches the component at render time.
2. Check if a refactor replaced `JiraLinkedText` with plain `<Markdown>` in some code paths.
3. Verify the regex `([A-Z][A-Z0-9]+-\d+)` matches the ticket key format being used.
4. Check if the `processChildren` function in the Markdown component is stripping or wrapping the JiraLinkedText output.

**Fix:** Depends on investigation. Likely a prop threading issue or a component replacement during recent refactors.

**Fallback improvement:** If `jiraBaseUrl` is empty but `externalRefs` exist on a ticket (from a previous push), use the stored URL directly.

**Files affected:**
- `src/components/JiraLinkedText.tsx` — potential fix
- `src/features/chat/ChatPanel.tsx` — verify JiraLinkedText usage in message rendering
- `src/features/ticket/TicketList.tsx` — verify external ref links

---

## 6. Global Search (Cmd+K)

**Problem:** No way to search across chats, notes, and docs.

**Frontend:**
- Built on existing `cmdk` dependency.
- `Cmd+K` opens a centered overlay command palette.
- Search input at top. Results grouped by type (Chats, Notes, Docs) with section headers.
- Each result: content preview (with match highlighted), source session/doc name.
- Click result → navigate to that session and scroll to match in context.
- Prev/Next buttons cycle through matches.
- Escape closes.

**Backend:**
- New Tauri command: `search_all(query: String) -> Vec<SearchResult>`.
- SearchResult: `{ content_type: "chat"|"note"|"doc", session_id, session_name, preview: String, match_offset: usize }`.
- Queries FTS5 across `conversation_messages`, `notes`, and `documents` tables.
- Uses FTS5 `snippet()` for highlighted excerpts.
- Add FTS5 indexes on tables that don't have them yet.

**Navigation:**
- For chat results: switch to the target session + conversation, scroll message into view with highlight.
- For note results: switch to session's dump view, scroll to match.
- For doc results: switch to docs view, open the document, scroll to section.

**Files affected:**
- New: `src/components/SearchPalette.tsx`
- `src/App.tsx` — add Cmd+K listener, render SearchPalette
- New: `src-tauri/src/search/` module (mod.rs, commands.rs, store.rs)
- `src-tauri/src/db.rs` — add FTS5 indexes in migration
- `src-tauri/src/lib.rs` — register new commands

---

## 7. Multi-Chat Per Session

**Problem:** Each session has one conversation. Can't start fresh without losing context (notes, tickets, repos).

**Data model:**
- New `conversations` table: `id` (UUID), `session_id` (FK), `title` (String), `created_at`, `updated_at`.
- Existing `conversation_messages` table gets `conversation_id` FK (replaces or supplements `session_id`).
- Each session has 1..N conversations.

**Migration:** Existing messages: create one conversation per session, move messages to it. Non-destructive.

**Auto-title:** "Chat 1", "Chat 2", etc. as default. After first assistant response, auto-generate a title: truncate first user message to its first sentence, max 40 characters. Simple string extraction, no LLM call for v1.

**UI — Sidebar:**
- Active session expands to show its conversations as indented sub-items.
- Click conversation to switch. Active one highlighted.
- `+ New Chat` at bottom of conversation list.

**UI — Chat Panel header:**
- Conversation title at top-left.
- `+ New Chat` button top-right.
- `...` menu: Rename conversation, Delete conversation.

**Behavior:**
- New conversation inherits session context (notes, tickets, repos) but starts with empty messages.
- Switching conversations preserves scroll position of the previous one.
- Deleting a conversation: confirm dialog if it has messages.

**Files affected:**
- `src-tauri/src/session/model.rs` — add Conversation struct (extend existing model file)
- `src-tauri/src/session/conversation_store.rs` — CRUD for conversations, migrate message queries
- `src-tauri/src/session/commands.rs` — new commands: create_conversation, list_conversations, rename_conversation, delete_conversation
- `src-tauri/src/db.rs` — migration for conversations table
- `src/features/session/SessionSidebar.tsx` — add conversation sub-list
- `src/features/session/session.atoms.ts` — add activeConversationId atom
- `src/features/chat/ChatPanel.tsx` — wire to active conversation instead of session
- `src/features/chat/chat.atoms.ts` — conversation-scoped message atoms

---

## 8. RAG Visibility + Smarter Search

**Problem:** RAG is invisible — no feedback on what context is used. Search requires explicit file mentions.

### Part A: Context Visibility

- Collapsible "Context" section under each assistant message that used RAG.
- Collapsed: "Used N files from M repos" (small, dimmed text).
- Expanded: list of file paths + line ranges included in context.
- Store retrieved chunks alongside each message: add `rag_context` JSON column to messages table.

### Part B: Smarter Search Query Extraction

- **Query expansion:** Before RAG search, extract search terms from user message. Simple heuristic (no LLM call): noun phrases, CamelCase identifiers, snake_case identifiers, quoted strings, @-mentions.
- **Conversation-aware:** Include terms from last 2-3 messages in search query, not just current message. Maintains topical context across turns.
- **Multi-pass search:** Already have semantic + FTS5 fusion. Improvement is in keyword expansion — feed expanded terms into FTS5 query.
- **Adaptive TOP_K:** Default 8 chunks. If message references multiple files or broad topics (heuristic: >3 distinct search terms), increase to 15.

**Files affected:**
- `src-tauri/src/rag/search.rs` — query expansion, conversation-aware context, adaptive TOP_K
- `src-tauri/src/llm/streaming.rs` — store rag_context with message, emit chunk details
- `src-tauri/src/session/conversation_store.rs` — add rag_context column to messages
- `src-tauri/src/db.rs` — migration for rag_context column
- `src/features/chat/ChatPanel.tsx` — render collapsible context section under assistant messages

---

## Implementation Order

Recommended priority (can be parallelized where independent):

1. **Chat input overhaul** — most basic interaction, everything depends on this being usable
2. **Stop generation** — unblocks daily use, pairs with input changes
3. **Markdown rendering** — readability of every response
4. **@ mention rework** — fixes broken file referencing
5. **Ticket link bug** — quick investigation/fix
6. **Multi-chat per session** — DB migration needed early, other features build on conversation model
7. **Global search (Cmd+K)** — depends on FTS5 indexes, benefits from multi-chat data model
8. **RAG improvements** — builds on conversation model and search infrastructure

---

## New Dependencies

**Rust (Cargo.toml):**
- `tokio-util` — for `CancellationToken`

**TypeScript (package.json):**
- `react-syntax-highlighter` — code block highlighting
- `textarea-caret` — textarea caret coordinate calculation (for @ mention positioning)

---

## Out of Scope

- Refine tab (user confirmed Docs tab covers the need)
- Visual companion / browser mockups
- RAG v2 (LLM-powered query expansion, re-ranking) — future iteration
- Board/Kanban view (Phase 3 in PLAN.md, separate spec)
