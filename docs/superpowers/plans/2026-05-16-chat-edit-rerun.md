# Chat Edit + Re-run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to edit a previous user chat message and re-run the conversation from that point, deleting everything after the edited message.

**Architecture:** One new backend store function + Tauri command for deleting messages from a point forward. Frontend edits to ChatPanel: edit mode state, pencil icon on hover, inline textarea, submit triggers delete + re-send.

**Tech Stack:** Rust (rusqlite), React 19, Jotai, Tailwind CSS v4, Lucide icons

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/src/session/conversation_store.rs` | Add `delete_from_message` function + tests |
| Modify | `src-tauri/src/session/commands.rs` | Add `delete_conversation_from` Tauri command |
| Modify | `src-tauri/src/lib.rs` | Register new command |
| Modify | `src/features/chat/ChatPanel.tsx` | Edit mode UI, pencil icon, submit flow |

---

### Task 1: Add `delete_from_message` to conversation store

**Files:**
- Modify: `src-tauri/src/session/conversation_store.rs`

- [ ] **Step 1: Write failing tests**

In `src-tauri/src/session/conversation_store.rs`, add to the `tests` module:

```rust
#[test]
fn delete_from_message_removes_target_and_later() {
    let db = test_db();
    let conn = db.conn().unwrap();

    conn.execute(
        "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
        params!["s1", "Test"],
    )
    .unwrap();

    save_message(&conn, "s1", "User", "First").unwrap();
    save_message(&conn, "s1", "Assistant", "Response 1").unwrap();
    save_message(&conn, "s1", "User", "Second").unwrap();
    save_message(&conn, "s1", "Assistant", "Response 2").unwrap();

    let messages = list_by_session(&conn, "s1").unwrap();
    assert_eq!(messages.len(), 4);

    let target_id = &messages[2].id; // "Second"
    let deleted = delete_from_message(&conn, "s1", target_id).unwrap();
    assert_eq!(deleted, 2);

    let remaining = list_by_session(&conn, "s1").unwrap();
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].content, "First");
    assert_eq!(remaining[1].content, "Response 1");
}

#[test]
fn delete_from_message_nonexistent_returns_error() {
    let db = test_db();
    let conn = db.conn().unwrap();

    conn.execute(
        "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
        params!["s1", "Test"],
    )
    .unwrap();

    let result = delete_from_message(&conn, "s1", "nonexistent");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test conversation_store --lib 2>&1`
Expected: Compilation error — `delete_from_message` does not exist.

- [ ] **Step 3: Implement `delete_from_message`**

In `src-tauri/src/session/conversation_store.rs`, add after the `list_by_session` function:

```rust
pub fn delete_from_message(conn: &Connection, session_id: &str, message_id: &str) -> AppResult<usize> {
    let created_at: String = conn
        .query_row(
            "SELECT created_at FROM conversations WHERE id = ?1 AND session_id = ?2",
            params![message_id, session_id],
            |row| row.get(0),
        )
        .map_err(|_| crate::error::AppError::Other(format!("Message {message_id} not found")))?;

    let deleted = conn.execute(
        "DELETE FROM conversations WHERE session_id = ?1 AND (id = ?2 OR created_at > ?3)",
        params![session_id, message_id, created_at],
    )?;

    Ok(deleted)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test conversation_store --lib 2>&1`
Expected: All 4 conversation_store tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/session/conversation_store.rs
git commit -m "feat: add delete_from_message to conversation store"
```

---

### Task 2: Add `delete_conversation_from` Tauri command

**Files:**
- Modify: `src-tauri/src/session/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the Tauri command**

In `src-tauri/src/session/commands.rs`, add after the `get_conversation` command:

```rust
#[tauri::command]
pub fn delete_conversation_from(
    db: State<Database>,
    session_id: String,
    message_id: String,
) -> Result<usize, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::delete_from_message(&conn, &session_id, &message_id)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in `lib.rs`**

In `src-tauri/src/lib.rs`, add `delete_conversation_from` to the `invoke_handler` list, after `get_conversation`:

```rust
get_conversation,
delete_conversation_from,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/session/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add delete_conversation_from Tauri command"
```

---

### Task 3: Add edit mode to ChatPanel

**Files:**
- Modify: `src/features/chat/ChatPanel.tsx`

- [ ] **Step 1: Add imports**

Add `Pencil` to the existing lucide-react imports (there are none currently — add the import):

```tsx
import { Pencil } from "lucide-react";
```

- [ ] **Step 2: Add edit state**

Inside the `ChatPanel` function, after the existing state declarations (after `const isExtracting = useAtomValue(isExtractingAtom);`), add:

```tsx
const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
const [editingContent, setEditingContent] = useState("");
```

- [ ] **Step 3: Add edit submit handler**

After the existing `handleSend` function, add:

```tsx
async function handleEditSubmit() {
  const text = editingContent.trim();
  if (!text || !activeSession || isStreaming) return;

  const messageId = editingMessageId;
  setEditingMessageId(null);
  setEditingContent("");
  setErrors([]);
  setIsStreaming(true);
  setStreamingContent("");
  shouldAutoScroll.current = true;

  await invoke("delete_conversation_from", {
    sessionId: activeSession.id,
    messageId,
  });

  const updated = await invoke<ConversationMessage[]>("get_conversation", {
    sessionId: activeSession.id,
  });
  setConversation(updated);

  await invoke("send_message", {
    sessionId: activeSession.id,
    content: text,
    mode: chatMode,
  });
}
```

- [ ] **Step 4: Update message rendering with edit mode**

Replace the message map block. Find:

```tsx
{conversation.map((msg) => (
  <div
    key={msg.id}
    className={`text-sm ${
      msg.role === "User"
        ? "ml-8 rounded-lg bg-accent/50 px-3 py-2"
        : "mr-4"
    }`}
  >
    {msg.role === "User" ? (
      <p className="whitespace-pre-wrap">{msg.content}</p>
    ) : (
      <div className="prose prose-invert prose-sm max-w-none">
        <Markdown remarkPlugins={[remarkGfm]}>{msg.content}</Markdown>
      </div>
    )}
  </div>
))}
```

Replace with:

```tsx
{conversation.map((msg) => {
  const isEditing = editingMessageId === msg.id;
  const isFaded = editingMessageId != null && !isEditing &&
    conversation.findIndex((m) => m.id === editingMessageId) <
    conversation.findIndex((m) => m.id === msg.id);

  return (
    <div
      key={msg.id}
      className={`text-sm group ${isFaded ? "opacity-40" : ""} ${
        msg.role === "User"
          ? "ml-8 rounded-lg bg-accent/50 px-3 py-2"
          : "mr-4"
      }`}
    >
      {msg.role === "User" && isEditing ? (
        <div className="space-y-2">
          <textarea
            autoFocus
            value={editingContent}
            onChange={(e) => setEditingContent(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleEditSubmit();
              }
              if (e.key === "Escape") {
                setEditingMessageId(null);
                setEditingContent("");
              }
            }}
            className="w-full rounded-md border border-input bg-background px-2 py-1 text-sm resize-none focus:outline-none focus:ring-1 focus:ring-ring"
            rows={3}
          />
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              size="xs"
              onClick={() => {
                setEditingMessageId(null);
                setEditingContent("");
              }}
            >
              Cancel
            </Button>
            <Button
              size="xs"
              onClick={handleEditSubmit}
              disabled={!editingContent.trim()}
            >
              Send
            </Button>
          </div>
        </div>
      ) : msg.role === "User" ? (
        <div className="relative">
          <p className="whitespace-pre-wrap">{msg.content}</p>
          {!isStreaming && !editingMessageId && (
            <button
              onClick={() => {
                setEditingMessageId(msg.id);
                setEditingContent(msg.content);
              }}
              className="absolute -top-1 -right-1 hidden group-hover:block p-0.5 rounded text-muted-foreground hover:text-foreground bg-accent"
              title="Edit and re-run"
            >
              <Pencil className="size-3" />
            </button>
          )}
        </div>
      ) : (
        <div className="prose prose-invert prose-sm max-w-none">
          <Markdown remarkPlugins={[remarkGfm]}>{msg.content}</Markdown>
        </div>
      )}
    </div>
  );
})}
```

- [ ] **Step 5: Disable input during edit mode**

Update the input disabled state. Find:

```tsx
disabled={isStreaming}
```

On the `<Input>` element in the form at the bottom, replace with:

```tsx
disabled={isStreaming || editingMessageId != null}
```

Also update the send button disabled state. Find:

```tsx
disabled={isStreaming || !inputValue.trim()}
```

Replace with:

```tsx
disabled={isStreaming || !inputValue.trim() || editingMessageId != null}
```

- [ ] **Step 6: Verify it compiles**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -10`
Expected: No TypeScript errors.

- [ ] **Step 7: Commit**

```bash
git add src/features/chat/ChatPanel.tsx
git commit -m "feat: add chat message edit and re-run"
```

---

### Task 4: Integration test

**Files:** None (testing only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests pass (55 total — 53 existing + 2 new conversation_store tests).

- [ ] **Step 2: Run frontend build check**

Run: `bun run build 2>&1 | tail -5`
Expected: Builds with no errors.
