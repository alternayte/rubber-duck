# Adversarial Review Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 12 issues found in the Phase 1 adversarial code review — 2 critical (path traversal, silent data loss), 6 important, 4 minor.

**Architecture:** All fixes are surgical changes to existing code. One new file (`session/conversation_store.rs`) centralizes conversation persistence. The rest are edits to existing files. No new dependencies.

**Tech Stack:** Rust (Tauri v2, rusqlite, reqwest, tracing), TypeScript (React, Jotai), Tailwind CSS

**Spec:** `docs/superpowers/specs/2026-05-15-adversarial-review-fixes-design.md`

---

## File Map

| Action | File | Purpose |
|--------|------|---------|
| Create | `src-tauri/src/session/conversation_store.rs` | Centralized conversation persistence (#4) |
| Modify | `src-tauri/src/session/mod.rs` | Register new module |
| Modify | `src-tauri/src/session/commands.rs` | Use conversation_store (#4) |
| Modify | `src-tauri/src/llm/streaming.rs` | Use conversation_store, fix silent data loss (#2, #4) |
| Modify | `src-tauri/src/lib.rs` | Path traversal, content-type, unwrap (#1, #5, #8) |
| Modify | `src-tauri/src/llm/context.rs` | UTF-8 safe truncation (#9) |
| Modify | `src-tauri/src/llm/client.rs` | HTTP timeout (#10) |
| Modify | `src-tauri/src/db.rs` | Dead code: `open_in_memory` to `#[cfg(test)]` (#11) |
| Modify | `src-tauri/src/session/store.rs` | Dead code: `#[allow(dead_code)]` on delete (#11) |
| Modify | `src-tauri/src/settings/store.rs` | Dead code: `#[allow(dead_code)]` on get_by_category (#11) |
| Modify | `src-tauri/tauri.conf.json` | Enable CSP (#3) |
| Modify | `src/main.tsx` | Jotai Provider (#12) |
| Modify | `src/features/chat/chat.atoms.ts` | Add isExtractingAtom (#7) |
| Modify | `src/features/chat/ChatPanel.tsx` | Skip llm:done when extracting (#7) |
| Modify | `src/features/session/DumpView.tsx` | Use isExtractingAtom (#7) |
| Modify | `src/features/ticket/TicketList.tsx` | Debounce textarea saves (#6) |

---

### Task 1: Create conversation store module

Centralizes all conversation SQL that's currently scattered in `streaming.rs` and `commands.rs`. Needed before Task 2.

**Files:**
- Create: `src-tauri/src/session/conversation_store.rs`
- Modify: `src-tauri/src/session/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/session/conversation_store.rs` with tests only:

```rust
use rusqlite::{params, Connection};

use crate::error::AppResult;
use super::model::ConversationMessage;

pub fn save_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
) -> AppResult<()> {
    todo!()
}

pub fn list_by_session(
    conn: &Connection,
    session_id: &str,
) -> AppResult<Vec<ConversationMessage>> {
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
    fn save_and_list() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        save_message(&conn, "s1", "User", "Hello duck").unwrap();
        save_message(&conn, "s1", "Assistant", "Quack").unwrap();

        let messages = list_by_session(&conn, "s1").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "User");
        assert_eq!(messages[0].content, "Hello duck");
        assert_eq!(messages[1].role, "Assistant");
        assert_eq!(messages[1].content, "Quack");
    }

    #[test]
    fn list_empty_session() {
        let db = test_db();
        let conn = db.conn().unwrap();

        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();

        let messages = list_by_session(&conn, "s1").unwrap();
        assert!(messages.is_empty());
    }
}
```

- [ ] **Step 2: Register the module**

Add to `src-tauri/src/session/mod.rs`:

```rust
pub mod conversation_store;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test conversation_store -- --nocapture 2>&1`

Expected: both tests fail with `not yet implemented`

- [ ] **Step 4: Implement save_message and list_by_session**

Replace the `todo!()` bodies in `conversation_store.rs`:

```rust
pub fn save_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, ?3, ?4)",
        params![id, session_id, role, content],
    )?;
    Ok(())
}

pub fn list_by_session(
    conn: &Connection,
    session_id: &str,
) -> AppResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at FROM conversations
         WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let messages = stmt
        .query_map(params![session_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test conversation_store -- --nocapture 2>&1`

Expected: 2 tests pass

- [ ] **Step 6: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1`

Expected: all 41 tests pass (39 existing + 2 new)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/session/conversation_store.rs src-tauri/src/session/mod.rs
git commit -m "refactor: add conversation_store module for centralized persistence"
```

---

### Task 2: Wire conversation store + fix silent data loss

Replace inline SQL in `streaming.rs` and `commands.rs` with conversation_store calls. Fix the `let _ =` silent error drop.

**Files:**
- Modify: `src-tauri/src/llm/streaming.rs`
- Modify: `src-tauri/src/session/commands.rs`

- [ ] **Step 1: Update streaming.rs imports**

In `src-tauri/src/llm/streaming.rs`, replace:

```rust
use crate::session::store as session_store;
use crate::session::note_store;
```

with:

```rust
use crate::session::conversation_store;
use crate::session::store as session_store;
use crate::session::note_store;
```

- [ ] **Step 2: Replace user message inline SQL with conversation_store**

In `streaming.rs`, replace lines 49-54:

```rust
        let user_msg_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, 'User', ?3)",
            params![user_msg_id, session_id, content],
        )
        .map_err(|e| e.to_string())?;
```

with:

```rust
        conversation_store::save_message(&conn, &session_id, "User", &content)
            .map_err(|e| e.to_string())?;
```

- [ ] **Step 3: Replace assistant message save + fix silent data loss**

In `streaming.rs`, replace lines 134-142:

```rust
        if !full_content.is_empty() {
            let db: State<Database> = app_clone.state::<Database>();
            if let Ok(conn) = db.conn() {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT INTO conversations (id, session_id, role, content) VALUES (?1, ?2, 'Assistant', ?3)",
                    params![id, db_clone_session_id, full_content],
                );
            };
        }
```

with:

```rust
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
            }
        }
```

- [ ] **Step 4: Clean up unused imports in streaming.rs**

Remove the now-unused `rusqlite::params` import from the top of `streaming.rs`:

```rust
use rusqlite::params;
```

Also remove unused `uuid` import if the `uuid::Uuid::new_v4()` calls were the only usage in this file. Check — the user message ID and assistant message ID generation were the only uuid uses. Both are now inside `conversation_store`, so remove:

```rust
// No longer needed in streaming.rs — uuid is used inside conversation_store
```

(Only remove if the compiler confirms they're unused.)

- [ ] **Step 5: Update commands.rs to use conversation_store**

In `src-tauri/src/session/commands.rs`, replace the `get_conversation` function (lines 64-89):

```rust
#[tauri::command]
pub fn get_conversation(
    db: State<Database>,
    session_id: String,
) -> Result<Vec<ConversationMessage>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, created_at FROM conversations
             WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let messages = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(messages)
}
```

with:

```rust
#[tauri::command]
pub fn get_conversation(
    db: State<Database>,
    session_id: String,
) -> Result<Vec<ConversationMessage>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::list_by_session(&conn, &session_id).map_err(|e| e.to_string())
}
```

- [ ] **Step 6: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1`

Expected: all 41 tests pass. No behavioral change — just centralized persistence.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/llm/streaming.rs src-tauri/src/session/commands.rs
git commit -m "fix: wire conversation_store, log+emit on save failure (#2, #4)"
```

---

### Task 3: Fix rdimg:// path traversal, content-type, and unwrap

Security fix for the URI scheme handler: validate paths, detect MIME type, remove unwrap().

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Replace the entire rdimg handler**

In `src-tauri/src/lib.rs`, replace lines 19-40:

```rust
        .register_uri_scheme_protocol("rdimg", |_ctx, request| {
            let uri = request.uri();
            let decoded = percent_encoding::percent_decode_str(uri.path())
                .decode_utf8_lossy()
                .to_string();
            // URI path starts with '/' on all platforms; strip it to get the absolute fs path
            let fs_path = if decoded.starts_with('/') {
                decoded[1..].to_string()
            } else {
                decoded
            };
            match std::fs::read(&fs_path) {
                Ok(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .body(bytes)
                    .unwrap(),
                Err(_) => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap(),
            }
        })
```

with:

```rust
        .register_uri_scheme_protocol("rdimg", |ctx, request| {
            let make_response = |status: u16, body: Vec<u8>, content_type: &str| {
                tauri::http::Response::builder()
                    .status(status)
                    .header("Content-Type", content_type)
                    .body(body)
                    .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
            };

            let uri = request.uri();
            let decoded = percent_encoding::percent_decode_str(uri.path())
                .decode_utf8_lossy()
                .to_string();
            let fs_path = if decoded.starts_with('/') {
                &decoded[1..]
            } else {
                &decoded
            };

            let images_root = match ctx.app_handle().path().app_data_dir() {
                Ok(dir) => dir.join("images"),
                Err(_) => return make_response(500, Vec::new(), "text/plain"),
            };

            let canonical = match std::fs::canonicalize(fs_path) {
                Ok(p) => p,
                Err(_) => return make_response(404, Vec::new(), "text/plain"),
            };

            let canonical_root = match std::fs::canonicalize(&images_root) {
                Ok(p) => p,
                Err(_) => return make_response(404, Vec::new(), "text/plain"),
            };

            if !canonical.starts_with(&canonical_root) {
                return make_response(403, Vec::new(), "text/plain");
            }

            let content_type = match canonical.extension().and_then(|e| e.to_str()) {
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                _ => "application/octet-stream",
            };

            match std::fs::read(&canonical) {
                Ok(bytes) => make_response(200, bytes, content_type),
                Err(_) => make_response(404, Vec::new(), "text/plain"),
            }
        })
```

- [ ] **Step 2: Add the path import**

Add to the top of `lib.rs` if not already present:

```rust
use tauri::Manager;
```

(Already present at line 8 — verify no additional imports needed. The `ctx.app_handle().path()` method comes from the `Manager` trait which is already imported.)

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1`

Expected: compiles with no errors

- [ ] **Step 4: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1`

Expected: all 41 tests pass (no tests directly exercise the URI handler, but compilation confirms correctness)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "fix: rdimg:// path traversal, content-type detection, remove unwrap (#1, #5, #8)"
```

---

### Task 4: Minor Rust fixes — UTF-8, timeout, dead code

Three independent small fixes batched into one task.

**Files:**
- Modify: `src-tauri/src/llm/context.rs`
- Modify: `src-tauri/src/llm/client.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/session/store.rs`
- Modify: `src-tauri/src/settings/store.rs`

- [ ] **Step 1: Write a failing test for UTF-8 truncation**

Add this test to the `#[cfg(test)] mod tests` block in `src-tauri/src/llm/context.rs`:

```rust
    #[test]
    fn truncates_long_descriptions_without_utf8_panic() {
        let description = "a]".repeat(60);
        let tickets = vec![(
            "Test".to_string(),
            "Task".to_string(),
            "High".to_string(),
            description,
        )];
        let messages = assemble_context(&ChatMode::Assist, "", "", &tickets, &[]);
        assert!(messages[0].content.contains("Test"));
        assert!(messages[0].content.contains("..."));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test truncates_long_descriptions -- --nocapture 2>&1`

Expected: FAIL — panics with "byte index 100 is not a char boundary"

- [ ] **Step 3: Fix the truncation**

In `src-tauri/src/llm/context.rs`, replace line 56-58:

```rust
            let desc_preview = if description.len() > 100 {
                format!("{}...", &description[..100])
            } else {
```

with:

```rust
            let desc_preview = if description.len() > 100 {
                let truncated: String = description.chars().take(100).collect();
                format!("{truncated}...")
            } else {
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd src-tauri && cargo test truncates_long_descriptions -- --nocapture 2>&1`

Expected: PASS

- [ ] **Step 5: Add HTTP timeout to OpenRouter client**

In `src-tauri/src/llm/client.rs`, add import at the top:

```rust
use std::time::Duration;
```

Then replace line 60:

```rust
    let client = Client::new();
```

with:

```rust
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Other(format!("Failed to create HTTP client: {e}")))?;
```

- [ ] **Step 6: Fix dead code warnings**

In `src-tauri/src/db.rs`, move `open_in_memory` into a `#[cfg(test)]` block. Replace lines 37-39:

```rust
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }
```

with:

```rust
    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }
```

In `src-tauri/src/session/store.rs`, add `#[allow(dead_code)]` above the `delete` function (line 73):

```rust
#[allow(dead_code)]
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
```

In `src-tauri/src/settings/store.rs`, add `#[allow(dead_code)]` above the `get_by_category` function (line 28):

```rust
#[allow(dead_code)]
pub fn get_by_category(conn: &Connection, category: &str) -> AppResult<Vec<(String, String)>> {
```

- [ ] **Step 7: Run full test suite and check for warnings**

Run: `cd src-tauri && cargo test 2>&1`

Expected: 42 tests pass (41 + 1 new UTF-8 test), no dead code warnings

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/llm/context.rs src-tauri/src/llm/client.rs src-tauri/src/db.rs src-tauri/src/session/store.rs src-tauri/src/settings/store.rs
git commit -m "fix: UTF-8 safe truncation, HTTP timeout, suppress dead code warnings (#9, #10, #11)"
```

---

### Task 5: Enable CSP

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Set the CSP**

In `src-tauri/tauri.conf.json`, replace lines 20-22:

```json
    "security": {
      "csp": null
    }
```

with:

```json
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' rdimg: asset: blob: data:; connect-src 'self' https://openrouter.ai"
    }
```

- [ ] **Step 2: Verify the app launches**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && bun tauri dev`

Verify:
- App window opens
- Session list loads
- Create a session, type in the editor
- Paste an image (if possible) — verify it renders
- Open browser devtools (right-click → Inspect) — check console for CSP violations

If CSP violations appear for legitimate app functionality, adjust the policy accordingly.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "fix: enable Content-Security-Policy (#3)"
```

---

### Task 6: Add Jotai Provider

**Files:**
- Modify: `src/main.tsx`

- [ ] **Step 1: Add Provider import and wrapper**

In `src/main.tsx`, replace the entire file:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { Provider } from "jotai";
import App from "./App";
import "./app.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Provider>
      <App />
    </Provider>
  </React.StrictMode>,
);
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && npx tsc --noEmit 2>&1`

Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/main.tsx
git commit -m "fix: wrap App in Jotai Provider (#12)"
```

---

### Task 7: Fix extract tickets race condition

Promote `extracting` state to a shared Jotai atom. ChatPanel skips `llm:done` when extraction is active.

**Files:**
- Modify: `src/features/chat/chat.atoms.ts`
- Modify: `src/features/chat/ChatPanel.tsx`
- Modify: `src/features/session/DumpView.tsx`

- [ ] **Step 1: Add isExtractingAtom**

In `src/features/chat/chat.atoms.ts`, add at the end:

```typescript
export const isExtractingAtom = atom(false);
```

- [ ] **Step 2: Update ChatPanel to skip llm:done when extracting**

In `src/features/chat/ChatPanel.tsx`, add the import for the new atom. Change the import line:

```typescript
import {
  chatModeAtom,
  conversationAtom,
  isExtractingAtom,
  isStreamingAtom,
  streamingContentAtom,
} from "./chat.atoms";
```

Add a ref to track the extracting state inside the component (after the existing ref declarations around line 36):

```typescript
  const isExtractingRef = useRef(false);
  const isExtracting = useAtomValue(isExtractingAtom);

  useEffect(() => {
    isExtractingRef.current = isExtracting;
  }, [isExtracting]);
```

Then in the `llm:done` listener (around line 68), add a guard:

```typescript
    listen<{ full_content: string }>("llm:done", () => {
      if (isExtractingRef.current) return;
      setIsStreaming(false);
      setStreamingContent("");
      if (activeSession) {
        invoke<ConversationMessage[]>("get_conversation", {
          sessionId: activeSession.id,
        }).then(setConversation);
      }
    }).then((unlisten) => unlisteners.push(unlisten));
```

- [ ] **Step 3: Update DumpView to use isExtractingAtom**

In `src/features/session/DumpView.tsx`, add imports:

```typescript
import { useAtom, useAtomValue } from "jotai";
```

Replace the existing `useAtomValue` import from jotai (line 6):

```typescript
import { useAtomValue } from "jotai";
```

with:

```typescript
import { useAtom, useAtomValue } from "jotai";
```

Add the atom import:

```typescript
import { chatModeAtom, isExtractingAtom, isStreamingAtom } from "@/features/chat/chat.atoms";
```

(This replaces the existing separate imports of `chatModeAtom` and `isStreamingAtom`.)

Remove the local `extracting` state and replace with the atom. Change:

```typescript
  const [extracting, setExtracting] = useState(false);
```

to:

```typescript
  const [extracting, setExtracting] = useAtom(isExtractingAtom);
```

Remove the `useState` import for `extracting` if it was the only `useState` usage — but `DumpView` still uses `useState` for `note`, `content`, `mode`, `extractError`, so keep the import.

- [ ] **Step 4: Verify TypeScript compiles**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && npx tsc --noEmit 2>&1`

Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/features/chat/chat.atoms.ts src/features/chat/ChatPanel.tsx src/features/session/DumpView.tsx
git commit -m "fix: prevent extract tickets listener racing with chat events (#7)"
```

---

### Task 8: Debounce ticket textarea saves

Use local state for description/acceptance_criteria, sync on blur.

**Files:**
- Modify: `src/features/ticket/TicketList.tsx`

- [ ] **Step 1: Add local state for expanded ticket editing**

In `src/features/ticket/TicketList.tsx`, add state for the expanded ticket's text fields. After the existing state declarations (around line 26), add:

```typescript
  const [editingDesc, setEditingDesc] = useState("");
  const [editingAC, setEditingAC] = useState("");
```

- [ ] **Step 2: Initialize local state when expanding a ticket**

Replace the expand/collapse toggle handler. Change the onClick in the expand button (around line 230):

```typescript
                <button
                  onClick={() => setExpandedId(expandedId === ticket.id ? null : ticket.id)}
```

to:

```typescript
                <button
                  onClick={() => {
                    if (expandedId === ticket.id) {
                      setExpandedId(null);
                    } else {
                      setExpandedId(ticket.id);
                      setEditingDesc(ticket.description);
                      setEditingAC(ticket.acceptance_criteria);
                    }
                  }}
```

- [ ] **Step 3: Replace textarea onChange handlers with local state + onBlur save**

Replace the description textarea (around lines 208-213):

```tsx
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={ticket.description}
                      onChange={(e) => updateTicket(ticket.id, sessionId, { description: e.target.value })}
                    />
```

with:

```tsx
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={editingDesc}
                      onChange={(e) => setEditingDesc(e.target.value)}
                      onBlur={() => {
                        if (editingDesc !== ticket.description) {
                          updateTicket(ticket.id, sessionId, { description: editingDesc });
                        }
                      }}
                    />
```

Replace the acceptance criteria textarea (around lines 218-221):

```tsx
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={ticket.acceptance_criteria}
                      onChange={(e) => updateTicket(ticket.id, sessionId, { acceptance_criteria: e.target.value })}
                    />
```

with:

```tsx
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={editingAC}
                      onChange={(e) => setEditingAC(e.target.value)}
                      onBlur={() => {
                        if (editingAC !== ticket.acceptance_criteria) {
                          updateTicket(ticket.id, sessionId, { acceptance_criteria: editingAC });
                        }
                      }}
                    />
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && npx tsc --noEmit 2>&1`

Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/features/ticket/TicketList.tsx
git commit -m "fix: debounce ticket textarea saves to onBlur (#6)"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run full backend test suite**

Run: `cd src-tauri && cargo test 2>&1`

Expected: 42 tests pass, no warnings

- [ ] **Step 2: Run TypeScript check**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && npx tsc --noEmit 2>&1`

Expected: no errors

- [ ] **Step 3: Run frontend build**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1`

Expected: build succeeds

- [ ] **Step 4: Launch app and smoke test**

Run: `bun tauri dev`

Verify:
1. App launches without CSP console errors
2. Can create session, type notes, switch Edit/Preview
3. Can send a chat message (if API key configured) — response streams and persists
4. Extract Tickets button works, tickets appear
5. Expand a ticket, edit description — saves only on blur, no lag while typing
6. No console errors in devtools
