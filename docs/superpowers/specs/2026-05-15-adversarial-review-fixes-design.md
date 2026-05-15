# Adversarial Review Fixes — Phase 1 Hardening

Addresses 12 issues found during the second adversarial code review of the Phase 1 implementation.

## Critical Fixes

### #1 — rdimg:// path traversal (local file disclosure)

**Problem:** The `rdimg://` URI scheme handler in `lib.rs:19-39` reads any file path without validation. Malicious markdown (from LLM output or imported sessions) could reference arbitrary local files.

**Fix:** Store the app data directory path in Tauri managed state at setup. In the handler, resolve the requested path and validate it starts with `{app_data}/images/`. Return 403 for anything outside that directory. Use `std::fs::canonicalize` to defeat `../` traversal.

**Files:** `src-tauri/src/lib.rs`

### #2 — Silent data loss on conversation save

**Problem:** `streaming.rs:138` uses `let _ = conn.execute(...)` when saving the assistant's response. DB errors are silently dropped — the user sees the message but it's never persisted.

**Fix:** Log errors via `tracing::error!` and emit an `llm:error` event with a user-facing message ("Message displayed but failed to save — try sending again"). The frontend already handles `llm:error` events.

**Files:** `src-tauri/src/llm/streaming.rs`

## Important Fixes

### #3 — CSP disabled

**Problem:** `tauri.conf.json` has `"csp": null`. No defense-in-depth against XSS if `rehype-raw` or similar is ever added.

**Fix:** Set CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' rdimg: asset: blob: data:; connect-src 'self' https://openrouter.ai`

- `unsafe-inline` for styles: required by Tailwind's runtime injection
- `rdimg:` in img-src: custom image protocol
- `blob:` and `data:` in img-src: CodeMirror and pasted images
- `connect-src` locked to OpenRouter only

**Files:** `src-tauri/tauri.conf.json`

### #4 — Conversation inserts scattered in LLM module

**Problem:** `streaming.rs` and `commands.rs` contain inline SQL for conversation persistence. This violates package-by-feature and will be missed when Phase 4 adds FTS indexing.

**Fix:** Create `session/conversation_store.rs` with:
- `save_message(conn, session_id, role, content) -> AppResult<()>`
- `list_by_session(conn, session_id) -> AppResult<Vec<ConversationMessage>>`

Update `streaming.rs` and `commands.rs` to call through the store.

**Files:** `src-tauri/src/session/conversation_store.rs` (new), `src-tauri/src/session/mod.rs`, `src-tauri/src/session/commands.rs`, `src-tauri/src/llm/streaming.rs`

### #5 — rdimg:// hardcodes Content-Type: image/png

**Problem:** `lib.rs:32` always returns `Content-Type: image/png` regardless of actual file type.

**Fix:** Detect content type from file extension: `.png` -> `image/png`, `.jpg`/`.jpeg` -> `image/jpeg`, `.gif` -> `image/gif`, `.webp` -> `image/webp`, fallback `application/octet-stream`.

**Files:** `src-tauri/src/lib.rs`

### #6 — Ticket textarea fires updateTicket every keystroke

**Problem:** `TicketList.tsx:210-221` calls `invoke("update_ticket")` on every `onChange`, causing IPC round-trip per keystroke.

**Fix:** Use local state for description/acceptance_criteria textareas. Sync to backend on blur, same pattern as title editing.

**Files:** `src/features/ticket/TicketList.tsx`

### #7 — Extract tickets listener race with chat events

**Problem:** Both DumpView and ChatPanel listen on `llm:done`. If a regular chat message completes while extraction is in flight (or vice versa), the wrong listener processes the response.

**Fix:** Promote `extracting` to a Jotai atom (`isExtractingAtom` in `chat.atoms.ts`). ChatPanel checks it and skips `llm:done` processing when true. DumpView sets it via the atom. This works because only one LLM request runs at a time in Phase 1.

**Files:** `src/features/chat/chat.atoms.ts`, `src/features/chat/ChatPanel.tsx`, `src/features/session/DumpView.tsx`

### #8 — unwrap() in non-test code

**Problem:** `lib.rs:34,38` uses `.unwrap()` on `Response::builder()`, violating the CLAUDE.md convention.

**Fix:** Replace with `unwrap_or_else(|_| Response::builder().status(500).body(Vec::new()).expect("static 500 response"))`. The inner `expect` is acceptable because the 500 response uses only static values.

**Files:** `src-tauri/src/lib.rs`

## Minor Fixes

### #9 — UTF-8 panic on description truncation

**Problem:** `context.rs:57` uses `&description[..100]` which panics if byte 100 falls inside a multi-byte character.

**Fix:** Replace with `description.chars().take(100).collect::<String>()`.

**Files:** `src-tauri/src/llm/context.rs`

### #10 — No HTTP timeout on OpenRouter request

**Problem:** `client.rs:60` uses `Client::new()` with no explicit timeout. A hanging server blocks indefinitely.

**Fix:** Use `Client::builder().timeout(Duration::from_secs(120)).build()`. 120s accommodates slow model responses while preventing infinite hangs.

**Files:** `src-tauri/src/llm/client.rs`

### #11 — Dead code warnings

**Problem:** Several functions are unused at the command level, generating compiler warnings.

**Fix:**
- `Database::open_in_memory`: move to `#[cfg(test)]` block
- `session::store::delete`, `settings::store::get_by_category`: add `#[allow(dead_code)]` (useful API surface not yet exposed)

**Files:** `src-tauri/src/db.rs`, `src-tauri/src/session/store.rs`, `src-tauri/src/settings/store.rs`

### #12 — No Jotai Provider

**Problem:** `main.tsx` relies on Jotai's default store. StrictMode double-renders or HMR could cause unexpected state sharing.

**Fix:** Wrap `<App />` in `<Provider>` from jotai.

**Files:** `src/main.tsx`
