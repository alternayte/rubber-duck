# Duck Chat Panel + Grill Mode — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the duck chat side panel to the send_message backend with streaming response display, conversation history, Assist/Grill mode toggle, and error handling.

**Architecture:** Backend gains ChatMode enum + grill system prompt in context.rs, mode parameter on send_message, and a get_conversation command. Frontend gets a ChatPanel component with Jotai atoms for streaming state, Tauri event listeners for llm:chunk/done/error, and markdown rendering on completion.

**Tech Stack:** Rust (context.rs, streaming.rs modifications), React (ChatPanel component, Jotai atoms, react-markdown, @tauri-apps/api event listeners)

**Spec:** `docs/superpowers/specs/2026-05-15-chat-panel-grill-mode-design.md`

---

## File Map

### Rust — Modify

| File | Change |
|------|--------|
| `src-tauri/src/llm/context.rs` | Add `ChatMode` enum, `GRILL_PROMPT`, `mode` param on `assemble_context` |
| `src-tauri/src/llm/streaming.rs` | Add `mode: String` param to `send_message`, pass to context |
| `src-tauri/src/session/model.rs` | Add `ConversationMessage` struct |
| `src-tauri/src/session/commands.rs` | Add `get_conversation` command |
| `src-tauri/src/lib.rs` | Register `get_conversation` in invoke_handler |

### Frontend — Create

| File | Responsibility |
|------|---------------|
| `src/features/chat/chat.types.ts` | ConversationMessage TypeScript type |
| `src/features/chat/chat.atoms.ts` | chatMode, isStreaming, streamingContent atoms |
| `src/features/chat/ChatPanel.tsx` | Chat UI: message list, input, streaming, mode toggle, errors |

### Frontend — Modify

| File | Change |
|------|--------|
| `src/App.tsx` | Replace placeholder chat section with ChatPanel |

---

## Task 1: Add ChatMode + grill prompt to context.rs

**Files:**
- Modify: `src-tauri/src/llm/context.rs`

- [ ] **Step 1: Add ChatMode enum and grill prompt**

Add before the existing `SYSTEM_PROMPT` const in `src-tauri/src/llm/context.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ChatMode {
    Assist,
    Grill,
}
```

Add after `SYSTEM_PROMPT`:

```rust
const GRILL_PROMPT: &str = "You are a critical technical reviewer. Your job is to find gaps, ambiguities, missing edge cases, and unstated assumptions in the user's planning session.

Read the current notes and tickets carefully. Then ask ONE focused question at a time. Be specific — reference actual content from their notes. Don't be generic.

Examples of good questions:
- \"You mention migrating the CDC pipeline but there's no ticket for schema migration — is that intentional or missing?\"
- \"The acceptance criteria for ticket #3 say 'handles errors gracefully' — what does that mean specifically? Which error cases?\"
- \"I see nothing about rollback strategy. What happens if this deployment fails halfway?\"

Do not provide solutions unless asked. Your job is to find the holes.";
```

- [ ] **Step 2: Add mode parameter to assemble_context**

Change the function signature:

```rust
pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    conversation: &[(String, String)],
) -> Vec<ChatMessage> {
```

Change the first line of the body from:

```rust
let mut system_parts = vec![SYSTEM_PROMPT.to_string()];
```

to:

```rust
let base_prompt = match mode {
    ChatMode::Assist => SYSTEM_PROMPT,
    ChatMode::Grill => GRILL_PROMPT,
};
let mut system_parts = vec![base_prompt.to_string()];
```

- [ ] **Step 3: Update existing tests to pass ChatMode::Assist**

Every existing test calls `assemble_context(...)` without a mode parameter. Add `&ChatMode::Assist` as the first argument to all 6 test calls. For example:

```rust
// Before:
let messages = assemble_context("", "", &[], &[]);
// After:
let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[]);
```

Apply this to all 6 tests: `system_message_includes_prompt`, `includes_session_context`, `includes_note_content`, `includes_tickets`, `includes_conversation_as_separate_messages`, `caps_conversation_at_40_messages`.

- [ ] **Step 4: Add grill mode test**

Add this test to the `mod tests` block:

```rust
#[test]
fn grill_mode_uses_different_prompt() {
    let messages = assemble_context(&ChatMode::Grill, "", "", &[], &[]);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].content.contains("critical technical reviewer"));
    assert!(!messages[0].content.contains("rubber-duck"));
}
```

- [ ] **Step 5: Verify tests pass**

Run: `cd src-tauri && cargo test llm::context::tests -v`
Expected: 7 tests pass (6 existing + 1 new)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/context.rs
git commit -m "feat: add ChatMode enum with grill system prompt"
```

---

## Task 2: Add mode param to send_message + get_conversation command

**Files:**
- Modify: `src-tauri/src/llm/streaming.rs`
- Modify: `src-tauri/src/session/model.rs`
- Modify: `src-tauri/src/session/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add mode parameter to send_message**

In `src-tauri/src/llm/streaming.rs`, change the `send_message` function signature:

```rust
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    db: State<'_, Database>,
    session_id: String,
    content: String,
    mode: String,
) -> Result<(), String> {
```

Add after the `api_key` line:

```rust
let chat_mode = if mode == "grill" {
    context::ChatMode::Grill
} else {
    context::ChatMode::Assist
};
```

Change the `assemble_context` call:

```rust
let messages = context::assemble_context(
    &chat_mode,
    &session.context,
    &note_content,
    &tickets,
    &conversation,
);
```

- [ ] **Step 2: Add ConversationMessage struct**

Add to `src-tauri/src/session/model.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
```

- [ ] **Step 3: Add get_conversation command**

Add to `src-tauri/src/session/commands.rs`:

Import the new type at the top (update the existing `use super::model::` line):

```rust
use super::model::{ConversationMessage, Note, Session};
```

Add the command:

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

- [ ] **Step 4: Register get_conversation in lib.rs**

Add `get_conversation` to the `invoke_handler` list in `src-tauri/src/lib.rs`, after `update_note`.

- [ ] **Step 5: Verify compilation and tests**

Run: `cd src-tauri && cargo test -v`
Expected: All tests pass (31 existing + 1 new grill test = 32)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/streaming.rs src-tauri/src/session/model.rs src-tauri/src/session/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add mode param to send_message and get_conversation command"
```

---

## Task 3: Chat atoms and types

**Files:**
- Create: `src/features/chat/chat.types.ts`
- Create: `src/features/chat/chat.atoms.ts`

- [ ] **Step 1: Create chat types**

Create `src/features/chat/chat.types.ts`:

```typescript
export interface ConversationMessage {
  id: string;
  role: "User" | "Assistant" | "System";
  content: string;
  created_at: string;
}
```

- [ ] **Step 2: Create chat atoms**

Create `src/features/chat/chat.atoms.ts`:

```typescript
import { atom } from "jotai";
import type { ConversationMessage } from "./chat.types";

export const chatModeAtom = atom<"assist" | "grill">("assist");
export const isStreamingAtom = atom(false);
export const streamingContentAtom = atom("");
export const conversationAtom = atom<ConversationMessage[]>([]);
```

- [ ] **Step 3: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Commit**

```bash
git add src/features/chat/
git commit -m "feat: add chat types and Jotai atoms"
```

---

## Task 4: ChatPanel component

**Files:**
- Create: `src/features/chat/ChatPanel.tsx`

- [ ] **Step 1: Create the ChatPanel component**

Create `src/features/chat/ChatPanel.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { apiKeySetAtom, settingsOpenAtom } from "@/features/settings/settings.atoms";
import {
  chatModeAtom,
  conversationAtom,
  isStreamingAtom,
  streamingContentAtom,
} from "./chat.atoms";
import type { ConversationMessage } from "./chat.types";

interface ErrorMessage {
  id: string;
  message: string;
}

export function ChatPanel() {
  const activeSession = useAtomValue(activeSessionAtom);
  const apiKeySet = useAtomValue(apiKeySetAtom);
  const setSettingsOpen = useSetAtom(settingsOpenAtom);
  const [chatMode, setChatMode] = useAtom(chatModeAtom);
  const [conversation, setConversation] = useAtom(conversationAtom);
  const [isStreaming, setIsStreaming] = useAtom(isStreamingAtom);
  const [streamingContent, setStreamingContent] = useAtom(streamingContentAtom);
  const [errors, setErrors] = useState<ErrorMessage[]>([]);
  const [inputValue, setInputValue] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const shouldAutoScroll = useRef(true);
  const listRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    if (shouldAutoScroll.current) {
      messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, []);

  function handleScroll() {
    const el = listRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 50;
    shouldAutoScroll.current = atBottom;
  }

  useEffect(() => {
    if (!activeSession) {
      setConversation([]);
      return;
    }
    invoke<ConversationMessage[]>("get_conversation", {
      sessionId: activeSession.id,
    }).then(setConversation);
  }, [activeSession?.id]);

  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<{ content: string }>("llm:chunk", (event) => {
      setStreamingContent((prev) => prev + event.payload.content);
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<{ full_content: string }>("llm:done", () => {
      setIsStreaming(false);
      setStreamingContent("");
      if (activeSession) {
        invoke<ConversationMessage[]>("get_conversation", {
          sessionId: activeSession.id,
        }).then(setConversation);
      }
    }).then((unlisten) => unlisteners.push(unlisten));

    listen<{ message: string }>("llm:error", (event) => {
      setIsStreaming(false);
      setStreamingContent("");
      const msg = event.payload.message;
      let displayMsg = msg;
      if (msg.includes("Keyring") || msg.includes("401")) {
        displayMsg = "Invalid API key — check Settings";
      } else if (msg.includes("429")) {
        displayMsg = "Rate limited — try again in a moment";
      } else if (msg.includes("connect") || msg.includes("network") || msg.includes("dns")) {
        displayMsg = "Connection failed — check your internet";
      }
      setErrors((prev) => [
        ...prev,
        { id: crypto.randomUUID(), message: displayMsg },
      ]);
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [activeSession?.id]);

  useEffect(() => {
    scrollToBottom();
  }, [conversation, streamingContent, errors]);

  async function handleSend() {
    const text = inputValue.trim();
    if (!text || !activeSession || isStreaming) return;

    setInputValue("");
    setErrors([]);
    setIsStreaming(true);
    setStreamingContent("");
    shouldAutoScroll.current = true;

    await invoke("send_message", {
      sessionId: activeSession.id,
      content: text,
      mode: chatMode,
    });
  }

  if (!activeSession) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center p-4">
        <p className="text-xs text-muted-foreground/60">
          Select a session to start chatting
        </p>
      </div>
    );
  }

  if (!apiKeySet) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 p-4">
        <p className="text-center text-xs text-muted-foreground">
          Set up your OpenRouter API key in Settings to start chatting
        </p>
        <Button size="xs" variant="secondary" onClick={() => setSettingsOpen(true)}>
          Open Settings
        </Button>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Header */}
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <h2 className="text-sm font-medium text-muted-foreground">Duck Chat</h2>
        <div className="ml-auto flex gap-1">
          <Button
            variant={chatMode === "assist" ? "secondary" : "ghost"}
            size="xs"
            onClick={() => setChatMode("assist")}
          >
            Assist
          </Button>
          <Button
            variant={chatMode === "grill" ? "secondary" : "ghost"}
            size="xs"
            onClick={() => setChatMode("grill")}
            className={chatMode !== "grill" ? "text-muted-foreground" : ""}
          >
            Grill
          </Button>
        </div>
      </div>

      {/* Message list */}
      <div
        ref={listRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-4 space-y-3"
      >
        {conversation.length === 0 && !isStreaming && (
          <p className="text-xs text-muted-foreground/60 text-center py-8">
            {chatMode === "grill"
              ? "Ask the duck to grill your plan"
              : "Ask the duck anything about your session"}
          </p>
        )}

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

        {isStreaming && streamingContent && (
          <div className="mr-4 text-sm">
            <p className="whitespace-pre-wrap">{streamingContent}</p>
          </div>
        )}

        {isStreaming && !streamingContent && (
          <div className="mr-4 text-sm">
            <p className="text-muted-foreground animate-pulse">Thinking...</p>
          </div>
        )}

        {errors.map((err) => (
          <div
            key={err.id}
            className="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive-foreground"
          >
            {err.message}
            {err.message.includes("Settings") && (
              <Button
                variant="link"
                size="xs"
                className="ml-2 text-destructive-foreground underline"
                onClick={() => setSettingsOpen(true)}
              >
                Open Settings
              </Button>
            )}
          </div>
        ))}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-border p-3">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            handleSend();
          }}
          className="flex gap-2"
        >
          <Input
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={
              chatMode === "grill"
                ? "Ask the duck to grill your plan..."
                : "Ask the duck..."
            }
            disabled={isStreaming}
            className="flex-1 text-sm"
          />
          <Button type="submit" size="sm" disabled={isStreaming || !inputValue.trim()}>
            Send
          </Button>
        </form>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: Clean

- [ ] **Step 3: Commit**

```bash
git add src/features/chat/ChatPanel.tsx
git commit -m "feat: implement ChatPanel with streaming and grill mode"
```

---

## Task 5: Wire ChatPanel into App.tsx

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Replace placeholder chat section with ChatPanel**

Add import at top of `src/App.tsx`:

```tsx
import { ChatPanel } from "@/features/chat/ChatPanel";
```

Replace the entire `{/* Chat section */}` block (lines 105-133 of current App.tsx — the `<div className="flex min-h-0 flex-1 flex-col">` that contains the Duck Chat header, message area, and input) with:

```tsx
<ChatPanel />
```

Also remove the `{/* Context section */}` div ABOVE the chat section stays — only the chat `div` gets replaced. The final side panel should look like:

```tsx
{sidePanelOpen && (
  <aside className="flex w-80 flex-col border-l border-border bg-card">
    {/* Context section */}
    <div className="border-b border-border p-4">
      <h2 className="text-sm font-medium text-muted-foreground">
        Context
      </h2>
      <p className="mt-2 text-xs text-muted-foreground/60">
        No repos or files attached
      </p>
    </div>

    <ChatPanel />
  </aside>
)}
```

- [ ] **Step 2: Clean up unused imports**

Remove the `Button` import from `@/components/ui/button` in App.tsx IF it's no longer used anywhere else in the file. Check if the tab bar or Panel toggle still use it — if so, keep it.

- [ ] **Step 3: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Verify Rust tests still pass**

Run: `cd src-tauri && cargo test -v`
Expected: 32 tests pass

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "feat: wire ChatPanel into App side panel"
```

---

## Task 6: Update PLAN.md

**Files:**
- Modify: `docs/PLAN.md`

- [ ] **Step 1: Check off Task 1.6**

Replace the Task 1.6 section in `docs/PLAN.md`:

```markdown
### Task 1.6 — Duck chat panel + Grill mode
- [x] Frontend: ChatPanel component with message list, streaming display, auto-scroll
- [x] Tauri event listeners: `llm:chunk`, `llm:done`, `llm:error` with error classification
- [x] Conversation history: `get_conversation` command, loads on session switch
- [x] Grill mode: `ChatMode` enum, alternate system prompt, Assist/Grill toggle
- [x] No-API-key state: prompts user to open settings
- [x] Error handling: inline error bubbles for API failures, rate limits, auth errors
```

- [ ] **Step 2: Commit**

```bash
git add docs/PLAN.md
git commit -m "docs: check off Task 1.6 in plan"
```
