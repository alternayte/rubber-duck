# Chat Message Edit + Re-run — Design Spec

## Overview

Allow users to edit a previous chat message and re-run the conversation from that point. Editing deletes the original message and everything after it, then sends the edited text as a new message through the normal `send_message` flow. No branch history — replace and delete forward.

## Scope

- Only user messages are editable
- No conversation branching or version history
- No confirmation dialog (Escape to cancel is sufficient)

## Backend Changes

### New store function: `delete_from_message`

In `src-tauri/src/session/conversation_store.rs`:

```rust
pub fn delete_from_message(conn: &Connection, session_id: &str, message_id: &str) -> AppResult<usize>
```

- Looks up the `created_at` of the given message
- Deletes the given message and all messages in the session with `created_at > that timestamp` (i.e. the message itself plus everything after it)
- Uses `WHERE session_id = ? AND (id = ? OR created_at > ?)` to handle the edge case where two messages share the same timestamp
- Returns the number of rows deleted
- If the message ID doesn't exist, returns an error

### New Tauri command: `delete_conversation_from`

In `src-tauri/src/session/commands.rs`:

```rust
#[tauri::command]
pub fn delete_conversation_from(db: State<Database>, session_id: String, message_id: String) -> Result<usize, String>
```

- Calls `conversation_store::delete_from_message`
- Registered in `lib.rs` invoke handler

## Frontend Changes

### ChatPanel edit mode

**Trigger:** Hover a user message → pencil icon appears (top-right corner of message bubble). Click to enter edit mode.

**Edit mode state:**
- `editingMessageId: string | null` — which message is being edited
- `editingContent: string` — current textarea content

**Edit mode UI:**
- Message text replaced with a textarea, pre-filled with original content
- Two buttons below: "Cancel" (ghost) and "Send" (primary)
- Bottom chat input is disabled while editing
- Messages after the edited one are visually faded (opacity-50)

**Submit flow:**
1. User presses Enter (without Shift) or clicks "Send"
2. Call `invoke("delete_conversation_from", { sessionId, messageId })`
3. Call `invoke("get_conversation", { sessionId })` to refresh (shows truncated history)
4. Call `invoke("send_message", { sessionId, content: editedContent, mode })` — reuses existing send flow
5. Exit edit mode

**Cancel flow:**
- Press Escape or click "Cancel"
- Restore original message display, clear edit state

**Keyboard shortcuts:**
- Enter (without Shift): submit edit
- Shift+Enter: newline in textarea
- Escape: cancel edit

### Interaction with streaming

- If the LLM is currently streaming (`isStreaming` is true), the edit icon should not appear
- If the user is in edit mode and streaming starts (shouldn't happen, but defensive), cancel edit mode

## Error Handling

- If `delete_conversation_from` fails: show inline error, stay in edit mode
- If `send_message` fails after truncation: normal error handling (the conversation is already truncated, but the error bubbles show)

## Out of Scope

- Editing assistant messages
- Conversation branching / version history
- Undo after edit+re-run
- Re-run without editing (just regenerate last response)
