# Duck Chat Panel + Grill Mode — Design Spec

## Overview

Wire the duck chat side panel to the existing `send_message` backend, add conversation history loading, streaming response display, Assist/Grill mode toggle, and error handling.

## Backend Changes

### Context Assembly (`llm/context.rs`)

Add chat mode enum and grill system prompt:

```rust
pub enum ChatMode {
    Assist,
    Grill,
}
```

`assemble_context` gains a `mode: ChatMode` parameter. When `Grill`, the system prompt becomes the critical reviewer persona from the PRD:

```
You are a critical technical reviewer. Your job is to find gaps, ambiguities, missing edge cases, and unstated assumptions in the user's planning session.

Read the current notes and tickets carefully. Then ask ONE focused question at a time. Be specific — reference actual content from their notes. Don't be generic.

Examples of good questions:
- "You mention migrating the CDC pipeline but there's no ticket for schema migration — is that intentional or missing?"
- "The acceptance criteria for ticket #3 say 'handles errors gracefully' — what does that mean specifically? Which error cases?"
- "I see nothing about rollback strategy. What happens if this deployment fails halfway?"

Do not provide solutions unless asked. Your job is to find the holes.
```

### send_message Command (`llm/streaming.rs`)

Add `mode: String` parameter to `send_message`. Map `"grill"` to `ChatMode::Grill`, default to `ChatMode::Assist`. Pass through to `assemble_context`.

### New Command: get_conversation

```rust
#[tauri::command]
pub fn get_conversation(db: State<Database>, session_id: String) -> Result<Vec<ConversationMessage>, String>
```

Returns all conversation messages for a session ordered by `created_at ASC`. The `ConversationMessage` struct:

```rust
#[derive(Serialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,       // "User", "Assistant", "System"
    pub content: String,
    pub created_at: String,
}
```

## Frontend Components

### `src/features/chat/chat.atoms.ts`

```typescript
chatModeAtom: atom<"assist" | "grill">("assist")
isStreamingAtom: atom(false)
streamingContentAtom: atom("")
```

### `src/features/chat/ChatPanel.tsx`

Replaces the placeholder chat section in the right side panel of App.tsx.

**Layout:**
- Header: "Duck Chat" + Assist/Grill toggle buttons
- Message list: scrollable, auto-scrolls to bottom on new messages
- Input bar: text input + send button at bottom, disabled while streaming

**Message rendering:**
- User messages: right-aligned or visually distinct (e.g., slightly different background)
- Assistant messages (complete): rendered as markdown via react-markdown
- Assistant messages (streaming): shown as plain text, replaced with markdown on completion
- Error messages: red-tinted inline bubbles
- Timestamps optional (can add later)

**Streaming flow:**
1. User submits message → call `send_message(session_id, content, mode)`
2. Set `isStreamingAtom = true`, clear `streamingContentAtom`
3. Listen to `llm:chunk` events → append to `streamingContentAtom`
4. Display streaming content as plain text in a pending bubble
5. On `llm:done` → set `isStreamingAtom = false`, reload conversation from DB via `get_conversation`
6. On `llm:error` → show error bubble, clear streaming state

**Conversation loading:**
- When `activeSessionAtom` changes, call `get_conversation(session_id)` and display history
- When no session selected, show nothing (chat panel is contextual to active session)

**No API key state:**
- Check `apiKeySetAtom` — if false, show "Set up your OpenRouter API key in Settings to start chatting" with a button that sets `settingsOpenAtom = true`
- Input is disabled when no API key

### Grill Mode Toggle

The Assist/Grill buttons in the chat header switch `chatModeAtom`. The active mode is visually highlighted (e.g., `variant="secondary"` vs `variant="ghost"`). The mode is passed to `send_message` on each call. Switching modes does NOT clear the conversation — both modes share the same conversation thread.

## Error Handling

| Tauri Event | Chat Display |
|-------------|-------------|
| `llm:error` with "Keyring error" | "Invalid API key — check Settings" + settings button |
| `llm:error` with "OpenRouter 429" | "Rate limited — try again in a moment" |
| `llm:error` with "OpenRouter 401" | "Invalid API key — check Settings" + settings button |
| `llm:error` with network text | "Connection failed — check your internet" |
| `llm:error` other | Show the raw error message |

Errors appear as inline messages in the chat (not modals/toasts). They're ephemeral — not saved to the conversation table.

## Out of Scope

- Message editing or deletion
- Regenerate last response
- Grill mode gap tracking (stretch goal in PLAN)
- Conversation branching
- Message search within chat
