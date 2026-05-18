# UX Polish Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 8 daily-use UX pain points: expandable chat input, stop generation, syntax-highlighted markdown, @ mention rework, ticket link bug, Cmd+K search, multi-chat per session, and RAG visibility.

**Architecture:** Each change is mostly independent. Multi-chat requires a DB migration that other features (search, RAG) build on, so it runs first among the "power features." Frontend changes touch `ChatPanel.tsx` heavily — tasks are ordered to avoid merge conflicts. Backend changes add a cancellation map to app state, a new `search` module, and extend the existing `rag` and `session` modules.

**Tech Stack:** Rust (tokio-util for CancellationToken), React 19, react-syntax-highlighter, textarea-caret, cmdk (already installed), Tailwind CSS, SQLite FTS5.

---

## File Map

### New Files
- `src/components/CodeBlock.tsx` — shared syntax-highlighted code block with copy button
- `src/components/SearchPalette.tsx` — Cmd+K global search overlay
- `src-tauri/src/search/mod.rs` — search module declaration
- `src-tauri/src/search/store.rs` — FTS5 queries for global search
- `src-tauri/src/search/commands.rs` — Tauri command for `search_all`
- `src-tauri/migrations/007_add_chat_threads.sql` — chat threads table + migration
- `src-tauri/migrations/008_add_conversation_fts.sql` — FTS5 on conversation content
- `src-tauri/migrations/009_add_rag_context.sql` — rag_context column on conversations

### Modified Files
- `src-tauri/Cargo.toml` — add `tokio-util`
- `package.json` — add `react-syntax-highlighter`, `textarea-caret`, `@types/react-syntax-highlighter`
- `src-tauri/src/db.rs` — register migrations 007-009
- `src-tauri/src/lib.rs` — add `CancellationTokens` state, `search` module, register new commands
- `src-tauri/src/llm/client.rs` — accept `CancellationToken`, check in stream loop
- `src-tauri/src/llm/streaming.rs` — create/store/check cancellation token, new `cancel_generation` command
- `src-tauri/src/docs/generator.rs` — same cancellation pattern for doc section generation
- `src-tauri/src/session/model.rs` — add `ChatThread` struct
- `src-tauri/src/session/conversation_store.rs` — thread-aware message CRUD
- `src-tauri/src/session/commands.rs` — new thread CRUD commands, update get_conversation
- `src-tauri/src/rag/search.rs` — query expansion, conversation-aware context, adaptive TOP_K
- `src/components/AtMentionInput.tsx` — rewrite: textarea + portal dropdown + caret positioning
- `src/components/JiraLinkedText.tsx` — bug fix (investigate + fix)
- `src/features/chat/ChatPanel.tsx` — stop button, CodeBlock, message copy, multi-chat, RAG context
- `src/features/chat/chat.atoms.ts` — add thread atoms, active thread
- `src/features/chat/chat.types.ts` — add ChatThread type
- `src/features/session/SessionSidebar.tsx` — thread sub-list under sessions
- `src/features/docs/DocumentCard.tsx` — stop button for section generation
- `src/App.tsx` — Cmd+K listener, render SearchPalette, `select-text` class fix

---

## Task 1: Install Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`

- [ ] **Step 1: Add tokio-util to Cargo.toml**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

- [ ] **Step 2: Install npm packages**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck
bun add react-syntax-highlighter textarea-caret
bun add -d @types/react-syntax-highlighter
```

- [ ] **Step 3: Verify builds**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck/src-tauri && cargo check 2>&1 | tail -5
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: both succeed.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml package.json bun.lockb
git commit -m "chore: add tokio-util, react-syntax-highlighter, textarea-caret"
```

---

## Task 2: Chat Input Overhaul

**Files:**
- Modify: `src/components/AtMentionInput.tsx`
- Modify: `src/App.tsx:70` (remove `select-none` to allow text selection)

- [ ] **Step 1: Replace Input with auto-resizing textarea in AtMentionInput.tsx**

Replace the entire content of `src/components/AtMentionInput.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { FileSearchResult } from "@/features/repo/repo.types";

interface AtMentionInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  sessionId: string;
  placeholder?: string;
  disabled?: boolean;
}

export function AtMentionInput({
  value,
  onChange,
  onSubmit,
  sessionId,
  placeholder,
  disabled,
}: AtMentionInputProps) {
  const [showDropdown, setShowDropdown] = useState(false);
  const [results, setResults] = useState<FileSearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStart, setMentionStart] = useState(-1);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const resize = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const maxH = window.innerHeight * 0.4;
    el.style.height = `${Math.min(el.scrollHeight, maxH)}px`;
    el.style.overflowY = el.scrollHeight > maxH ? "auto" : "hidden";
  }, []);

  useEffect(() => {
    resize();
  }, [value, resize]);

  const search = useCallback(
    (query: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        if (query.length < 1) {
          setResults([]);
          return;
        }
        const res = await invoke<FileSearchResult[]>("search_repo_files", {
          sessionId,
          query,
        });
        setResults(res);
        setSelectedIndex(0);
      }, 200);
    },
    [sessionId],
  );

  function handleChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    const newValue = e.target.value;
    onChange(newValue);

    const cursorPos = e.target.selectionStart ?? newValue.length;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf("@");

    if (atIndex >= 0 && (atIndex === 0 || textBeforeCursor[atIndex - 1] === " " || textBeforeCursor[atIndex - 1] === "\n")) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(" ") && !query.includes("\n")) {
        setMentionStart(atIndex);
        setMentionQuery(query);
        setShowDropdown(true);
        search(query);
        return;
      }
    }

    setShowDropdown(false);
  }

  function selectResult(result: FileSearchResult) {
    const before = value.slice(0, mentionStart);
    const after = value.slice(mentionStart + 1 + mentionQuery.length);
    const newValue = `${before}@${result.display}${after} `;
    onChange(newValue);
    setShowDropdown(false);
    textareaRef.current?.focus();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (showDropdown && results.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        selectResult(results[selectedIndex]);
        return;
      }
      if (e.key === "Escape") {
        setShowDropdown(false);
        return;
      }
    }

    if (e.key === "Enter" && !e.shiftKey && !showDropdown) {
      e.preventDefault();
      onSubmit();
    }
  }

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  return (
    <div className="relative flex-1">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        rows={1}
        className="w-full resize-none rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
        style={{ overflowY: "hidden" }}
      />
      {showDropdown && results.length > 0 && (
        <div className="absolute bottom-full left-0 right-0 mb-1 max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md z-50">
          {results.map((result, i) => {
            const lastSlash = result.display.lastIndexOf("/");
            const dir = lastSlash > 0 ? result.display.slice(0, lastSlash + 1) : "";
            const filename = lastSlash > 0 ? result.display.slice(lastSlash + 1) : result.display;
            return (
              <button
                key={result.display}
                onClick={() => selectResult(result)}
                className={`w-full text-left px-3 py-1.5 text-xs flex items-baseline gap-1 ${
                  i === selectedIndex
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-accent/50"
                }`}
              >
                <span className="text-muted-foreground/60 truncate">{dir}</span>
                <span className="text-foreground font-medium shrink-0">{filename}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Fix select-none on root div in App.tsx**

In `src/App.tsx:70`, change `select-none` to `select-text` so users can select text in messages:

```tsx
// Before:
<div className="flex h-screen bg-background text-foreground select-none">
// After:
<div className="flex h-screen bg-background text-foreground">
```

- [ ] **Step 3: Run dev server and test**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully. Manual test: textarea should auto-expand, Enter sends, Shift+Enter adds newline.

- [ ] **Step 4: Commit**

```bash
git add src/components/AtMentionInput.tsx src/App.tsx
git commit -m "feat: replace single-line chat input with auto-expanding textarea"
```

---

## Task 3: Stop Generation — Backend

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/llm/client.rs`
- Modify: `src-tauri/src/llm/streaming.rs`
- Modify: `src-tauri/src/docs/generator.rs`

- [ ] **Step 1: Add CancellationTokens state to lib.rs**

In `src-tauri/src/lib.rs`, add imports at the top:

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;
```

Add the state struct before the `run()` function:

```rust
pub struct CancellationTokens {
    pub tokens: Mutex<HashMap<String, CancellationToken>>,
}
```

In the `setup` closure (after `app.manage(db);`), add:

```rust
app.manage(CancellationTokens {
    tokens: Mutex::new(HashMap::new()),
});
```

In the `invoke_handler`, add after `llm::streaming::send_message`:

```rust
llm::streaming::cancel_generation,
```

- [ ] **Step 2: Add CancellationToken support to client.rs**

In `src-tauri/src/llm/client.rs`, add the import:

```rust
use tokio_util::sync::CancellationToken;
```

Change the `stream_completion` signature to accept a token:

```rust
pub async fn stream_completion(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: mpsc::Sender<StreamEvent>,
    cancel: CancellationToken,
) {
    let result = stream_inner(api_key, model, messages, &tx, &cancel).await;
    if let Err(e) = result {
        let _ = tx.send(StreamEvent::Error(e.to_string())).await;
    }
}
```

Change `stream_inner` to accept and check the token:

```rust
async fn stream_inner(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: &mpsc::Sender<StreamEvent>,
    cancel: &CancellationToken,
) -> AppResult<()> {
```

In the `while let Some(chunk) = stream.next().await` loop, add a cancellation check at the top of each iteration. Replace the while loop with:

```rust
    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = tx.send(StreamEvent::Done(full_content)).await;
                return Ok(());
            }
            chunk = stream.next() => chunk,
        };

        let Some(chunk) = chunk else {
            break;
        };

        let chunk = chunk.map_err(|e| AppError::Other(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                return Ok(());
            }

            match serde_json::from_str::<StreamResponse>(data) {
                Ok(parsed) => {
                    if let Some(choice) = parsed.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            full_content.push_str(content);
                            let _ = tx.send(StreamEvent::Chunk(content.clone())).await;
                        }
                        if choice.finish_reason.is_some() {
                            let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                            return Ok(());
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }

    let _ = tx.send(StreamEvent::Done(full_content)).await;
    Ok(())
```

- [ ] **Step 3: Update streaming.rs to use cancellation**

In `src-tauri/src/llm/streaming.rs`, add imports:

```rust
use tokio_util::sync::CancellationToken;
use crate::CancellationTokens;
```

In the `send_message` function, after the `(tx, mut rx)` channel creation (line 235), create and store a cancellation token:

```rust
    let cancel = CancellationToken::new();
    {
        let tokens: State<CancellationTokens> = app.state::<CancellationTokens>();
        let mut map = tokens.tokens.lock().unwrap();
        map.insert(session_id.clone(), cancel.clone());
    }
```

Pass the token to the streaming spawn (the first `tokio::spawn`):

```rust
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        client::stream_completion(&api_key, &model, messages, tx, cancel_clone).await;
    });
```

In the receiver spawn, after the `while let Some(event) = rx.recv().await` loop finishes, clean up the token:

```rust
        // Clean up cancellation token
        let tokens: State<CancellationTokens> = app_clone.state::<CancellationTokens>();
        if let Ok(mut map) = tokens.tokens.lock() {
            map.remove(&db_clone_session_id);
        }
```

Add the `cancel_generation` command at the bottom of streaming.rs:

```rust
#[tauri::command]
pub fn cancel_generation(
    app: AppHandle,
    session_id: String,
) -> Result<(), String> {
    let tokens: State<CancellationTokens> = app.state::<CancellationTokens>();
    let map = tokens.tokens.lock().map_err(|e| e.to_string())?;
    if let Some(token) = map.get(&session_id) {
        token.cancel();
    }
    Ok(())
}
```

- [ ] **Step 4: Update generator.rs for doc section cancellation**

In `src-tauri/src/docs/generator.rs`, add the import:

```rust
use tokio_util::sync::CancellationToken;
use crate::CancellationTokens;
```

In `generate_section`, after the `(tx, mut rx)` channel creation (line 241), add:

```rust
    let cancel = CancellationToken::new();
    {
        let tokens: State<CancellationTokens> = app.state::<CancellationTokens>();
        let mut map = tokens.tokens.lock().unwrap();
        map.insert(section_id.clone(), cancel.clone());
    }
```

Pass the token to the streaming spawn:

```rust
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        client::stream_completion(&api_key, &model, messages, tx, cancel_clone).await;
    });
```

Clean up in the receiver spawn after the loop:

```rust
        let tokens: State<CancellationTokens> = app_clone.state::<CancellationTokens>();
        if let Ok(mut map) = tokens.tokens.lock() {
            map.remove(&section_id_clone);
        }
```

Also register `cancel_generation` in `lib.rs` `invoke_handler` (add `docs::generator::cancel_doc_generation` — but since we reuse the same `CancellationTokens` map, the existing `cancel_generation` works for both by passing the section_id as the key). No additional command needed — the frontend will call `cancel_generation` with either the session_id or section_id.

- [ ] **Step 5: Verify compilation**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck/src-tauri && cargo check 2>&1 | tail -10
```

Expected: compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/llm/client.rs src-tauri/src/llm/streaming.rs src-tauri/src/docs/generator.rs src-tauri/Cargo.toml
git commit -m "feat: add generation cancellation with CancellationToken"
```

---

## Task 4: Stop Generation — Frontend

**Files:**
- Modify: `src/features/chat/ChatPanel.tsx`
- Modify: `src/features/docs/DocumentCard.tsx`

- [ ] **Step 1: Add stop button to ChatPanel**

In `src/features/chat/ChatPanel.tsx`, add the import:

```tsx
import { Pencil, Square } from "lucide-react";
```

(Replace the existing `import { Pencil } from "lucide-react";`)

In the `handleSend` function, add `activeSession.id` as context for the cancel:

Add a `handleCancel` function after `handleEditSubmit`:

```tsx
  async function handleCancel() {
    if (!activeSession) return;
    await invoke("cancel_generation", { sessionId: activeSession.id });
  }
```

In the input area (the `{/* Input */}` section at the bottom), replace the Send button with a conditional:

```tsx
          {isStreaming ? (
            <Button
              type="button"
              size="sm"
              variant="destructive"
              onClick={handleCancel}
            >
              <Square className="size-3.5 fill-current" />
            </Button>
          ) : (
            <Button
              type="button"
              size="sm"
              onClick={handleSend}
              disabled={!inputValue.trim() || editingMessageId != null}
            >
              Send
            </Button>
          )}
```

- [ ] **Step 2: Add stop button to DocumentCard section generation**

In `src/features/docs/DocumentCard.tsx`, add the import:

```tsx
import { invoke } from "@tauri-apps/api/core";
import { Square } from "lucide-react";
```

(Add `Square` to the existing lucide-react import, and add the invoke import)

In the `SectionRow` component, next to the Regenerate button (line ~229), add a conditional stop button:

```tsx
          {isGenerating ? (
            <Button
              size="xs"
              variant="ghost"
              onClick={() => invoke("cancel_generation", { sessionId: section.id })}
              className="size-6 p-0 text-destructive"
              title="Stop generating"
            >
              <Square className="size-3 fill-current" />
            </Button>
          ) : (
            <Button
              size="xs"
              variant="ghost"
              onClick={onRegenerate}
              disabled={isGenerating}
              className="size-6 p-0 text-muted-foreground"
              title="Regenerate"
            >
              <RefreshCw className={`size-3 ${isGenerating ? "animate-spin" : ""}`} />
            </Button>
          )}
```

This replaces the existing Regenerate button block.

- [ ] **Step 3: Add llm:cancelled event handler in ChatPanel**

In ChatPanel's `useEffect` with the event listeners, add a handler for cancellation:

```tsx
      listen<Record<string, never>>("llm:cancelled", () => {
        setIsStreaming(false);
        setStreamingContent("");
        setRagContext(null);
        if (activeSession) {
          invoke<ConversationMessage[]>("get_conversation", {
            sessionId: activeSession.id,
          }).then(setConversation);
        }
      }),
```

Note: The backend currently emits `llm:done` on cancel (the `StreamEvent::Done` path in the cancel branch). The frontend already handles `llm:done`. So this step may not be strictly needed — but adding it provides a dedicated handler if we later want to distinguish cancelled vs completed.

- [ ] **Step 4: Build and verify**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully.

- [ ] **Step 5: Commit**

```bash
git add src/features/chat/ChatPanel.tsx src/features/docs/DocumentCard.tsx
git commit -m "feat: add stop generation button to chat and doc generation"
```

---

## Task 5: Markdown Rendering + Message Actions

**Files:**
- Create: `src/components/CodeBlock.tsx`
- Modify: `src/features/chat/ChatPanel.tsx`
- Modify: `src/features/docs/DocumentCard.tsx`
- Modify: `src/features/session/DumpView.tsx`

- [ ] **Step 1: Create CodeBlock component**

Create `src/components/CodeBlock.tsx`:

```tsx
import { useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import { Check, Copy } from "lucide-react";

interface CodeBlockProps {
  language: string | undefined;
  children: string;
}

export function CodeBlock({ language, children }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(children);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="group/code relative my-2 rounded-md overflow-hidden">
      <div className="flex items-center justify-between bg-zinc-800 px-3 py-1 text-xs text-zinc-400">
        <span>{language ?? "text"}</span>
        <button onClick={handleCopy} className="flex items-center gap-1 hover:text-zinc-200">
          {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
          {copied ? "Copied!" : "Copy"}
        </button>
      </div>
      <SyntaxHighlighter
        language={language ?? "text"}
        style={oneDark}
        customStyle={{ margin: 0, borderRadius: 0, fontSize: "0.8rem" }}
      >
        {children}
      </SyntaxHighlighter>
    </div>
  );
}
```

- [ ] **Step 2: Create markdown components object for reuse**

In `src/features/chat/ChatPanel.tsx`, add the CodeBlock import at the top:

```tsx
import { CodeBlock } from "@/components/CodeBlock";
import { Check, Copy, Pencil, Square } from "lucide-react";
```

Add a reusable markdown components factory above the `ChatPanel` function:

```tsx
function markdownComponents(processChildrenFn: typeof processChildren) {
  return {
    p: ({ children }: { children?: React.ReactNode }) => <p>{processChildrenFn(children)}</p>,
    li: ({ children }: { children?: React.ReactNode }) => <li>{processChildrenFn(children)}</li>,
    code: ({ className, children, ...props }: React.ComponentPropsWithoutRef<"code"> & { inline?: boolean }) => {
      const match = /language-(\w+)/.exec(className || "");
      const codeString = String(children).replace(/\n$/, "");
      if (match) {
        return <CodeBlock language={match[1]}>{codeString}</CodeBlock>;
      }
      return (
        <code className="rounded bg-muted px-1 py-0.5 text-sm font-mono" {...props}>
          {children}
        </code>
      );
    },
  };
}
```

Update the assistant message Markdown usage (around line 337) to use the new components:

```tsx
                <div className="prose prose-invert prose-sm max-w-none">
                  <Markdown
                    remarkPlugins={[remarkGfm]}
                    components={markdownComponents(processChildren)}
                  >{msg.content}</Markdown>
                </div>
```

Also update the streaming content display (around line 358) to render as markdown:

```tsx
        {isStreaming && streamingContent && (
          <div className="mr-4 text-sm">
            <div className="prose prose-invert prose-sm max-w-none">
              <Markdown
                remarkPlugins={[remarkGfm]}
                components={markdownComponents(processChildren)}
              >{streamingContent}</Markdown>
            </div>
          </div>
        )}
```

- [ ] **Step 3: Add message-level copy button**

In the message rendering in ChatPanel, for assistant messages, wrap the content in a relative container and add a copy button. Replace the assistant message block with:

```tsx
              ) : (
                <div className="relative group/msg">
                  <button
                    onClick={() => navigator.clipboard.writeText(msg.content)}
                    className="absolute top-0 right-0 hidden group-hover/msg:flex items-center gap-1 rounded bg-accent px-1.5 py-0.5 text-xs text-muted-foreground hover:text-foreground"
                  >
                    <Copy className="size-3" /> Copy
                  </button>
                  <div className="prose prose-invert prose-sm max-w-none">
                    <Markdown
                      remarkPlugins={[remarkGfm]}
                      components={markdownComponents(processChildren)}
                    >{msg.content}</Markdown>
                  </div>
                </div>
              )}
```

- [ ] **Step 4: Apply CodeBlock to DumpView.tsx and DocumentCard.tsx**

In `src/features/session/DumpView.tsx`, add the import:

```tsx
import { CodeBlock } from "@/components/CodeBlock";
```

Update the Markdown components in DumpView to include the code block handling. Find the Markdown usage and update its components prop to include the code component (same pattern as ChatPanel).

In `src/features/docs/DocumentCard.tsx`, do the same: import CodeBlock and add the code component to the Markdown in SectionRow.

- [ ] **Step 5: Build and verify**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully.

- [ ] **Step 6: Commit**

```bash
git add src/components/CodeBlock.tsx src/features/chat/ChatPanel.tsx src/features/session/DumpView.tsx src/features/docs/DocumentCard.tsx
git commit -m "feat: add syntax-highlighted code blocks and message copy buttons"
```

---

## Task 6: @ Mention Portal Rework

**Files:**
- Modify: `src/components/AtMentionInput.tsx`

This task upgrades the dropdown from Task 2's implementation to use a React portal for proper positioning.

- [ ] **Step 1: Add portal rendering and caret-based positioning**

Update `src/components/AtMentionInput.tsx` — add portal imports and caret positioning:

```tsx
import { createPortal } from "react-dom";
import getCaretCoordinates from "textarea-caret";
```

Add state for dropdown position:

```tsx
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number } | null>(null);
```

In `handleChange`, when the dropdown is shown, calculate position from the textarea caret:

```tsx
    if (atIndex >= 0 && (atIndex === 0 || textBeforeCursor[atIndex - 1] === " " || textBeforeCursor[atIndex - 1] === "\n")) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(" ") && !query.includes("\n")) {
        setMentionStart(atIndex);
        setMentionQuery(query);
        setShowDropdown(true);
        search(query);

        // Calculate caret position for portal
        const el = textareaRef.current;
        if (el) {
          const coords = getCaretCoordinates(el, atIndex);
          const rect = el.getBoundingClientRect();
          setDropdownPos({
            top: rect.top + coords.top - el.scrollTop,
            left: rect.left + coords.left,
          });
        }
        return;
      }
    }
```

Replace the dropdown JSX with a portal. Remove the old dropdown div and replace with:

```tsx
      {showDropdown && results.length > 0 && dropdownPos &&
        createPortal(
          <div
            className="fixed max-h-48 w-72 overflow-y-auto rounded-md border border-border bg-popover shadow-md z-[100]"
            style={{
              top: dropdownPos.top - 4,
              left: dropdownPos.left,
              transform: "translateY(-100%)",
            }}
          >
            {results.map((result, i) => {
              const lastSlash = result.display.lastIndexOf("/");
              const dir = lastSlash > 0 ? result.display.slice(0, lastSlash + 1) : "";
              const filename = lastSlash > 0 ? result.display.slice(lastSlash + 1) : result.display;
              return (
                <button
                  key={result.display}
                  onClick={() => selectResult(result)}
                  className={`w-full text-left px-3 py-1.5 text-xs flex items-baseline gap-1 ${
                    i === selectedIndex
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50"
                  }`}
                >
                  <span className="text-muted-foreground/60 truncate">{dir}</span>
                  <span className="text-foreground font-medium shrink-0">{filename}</span>
                </button>
              );
            })}
          </div>,
          document.body,
        )}
```

- [ ] **Step 2: Build and verify**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully.

- [ ] **Step 3: Commit**

```bash
git add src/components/AtMentionInput.tsx
git commit -m "feat: render @ mention dropdown as portal with caret positioning"
```

---

## Task 7: Ticket Link Bug Investigation & Fix

**Files:**
- Modify: `src/components/JiraLinkedText.tsx` (likely)
- Modify: `src/features/chat/ChatPanel.tsx` (likely)

- [ ] **Step 1: Investigate the bug**

Check the following:

1. In `ChatPanel.tsx`, the `LinkedText` component is used inside `processChildren`. It splits text on mention patterns first, then passes non-mention parts to `JiraLinkedText`. Verify `JiraLinkedText` receives the right text.

2. `JiraLinkedText` reads `jiraBaseUrlAtom` via `useAtomValue`. Check that this atom is being set. In `App.tsx:41-43`, the Jira config is loaded and `setJiraBaseUrl(config.base_url)` is called. If `config.base_url` is empty string `""`, it will be falsy and links won't render.

3. Check if the `processChildren` function in the Markdown component is being called correctly. The Markdown component only wraps `p` and `li` children — but code blocks, headings, and other elements don't get processed. Ticket keys inside headings or blockquotes won't be linked.

Run this to check:

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck
grep -n "processChildren\|JiraLinkedText\|LinkedText" src/features/chat/ChatPanel.tsx
grep -n "jiraBaseUrl" src/features/settings/settings.atoms.ts src/App.tsx src/components/JiraLinkedText.tsx
```

- [ ] **Step 2: Apply fix based on investigation**

Most likely fix: the `jiraBaseUrlAtom` is initialized as `null` and the config loading in App.tsx may not set it correctly (e.g., if the config returns `base_url: ""` instead of `null`). Ensure the atom gets a valid URL.

Also extend `processChildren` to cover more Markdown elements. Add to the components object:

```tsx
    strong: ({ children }: { children?: React.ReactNode }) => <strong>{processChildrenFn(children)}</strong>,
    em: ({ children }: { children?: React.ReactNode }) => <em>{processChildrenFn(children)}</em>,
    a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
      <a href={href} target="_blank" rel="noopener noreferrer">{processChildrenFn(children)}</a>
    ),
```

If the base URL is the issue, add a fallback in `JiraLinkedText.tsx` — if `jiraBaseUrl` is falsy but the text matches a ticket pattern, still render it styled (just not clickable), so the user knows something is wrong with config.

- [ ] **Step 3: Commit**

```bash
git add src/components/JiraLinkedText.tsx src/features/chat/ChatPanel.tsx
git commit -m "fix: restore Jira ticket hyperlinking in chat messages"
```

---

## Task 8: Multi-Chat Per Session — Database & Backend

**Files:**
- Create: `src-tauri/migrations/007_add_chat_threads.sql`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/session/model.rs`
- Modify: `src-tauri/src/session/conversation_store.rs`
- Modify: `src-tauri/src/session/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/llm/streaming.rs`

- [ ] **Step 1: Write the migration**

Create `src-tauri/migrations/007_add_chat_threads.sql`:

```sql
-- Create chat_threads table
CREATE TABLE chat_threads (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT 'Chat 1',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_chat_threads_session ON chat_threads(session_id);

-- Add thread_id column to conversations (messages) table
ALTER TABLE conversations ADD COLUMN thread_id TEXT REFERENCES chat_threads(id) ON DELETE CASCADE;

-- Migrate: create one thread per session and assign all existing messages
INSERT INTO chat_threads (id, session_id, title, created_at)
SELECT
    'thread-' || s.id,
    s.id,
    'Chat 1',
    s.created_at
FROM sessions s
WHERE EXISTS (SELECT 1 FROM conversations c WHERE c.session_id = s.id);

-- Assign existing messages to their session's default thread
UPDATE conversations SET thread_id = 'thread-' || session_id
WHERE thread_id IS NULL;
```

- [ ] **Step 2: Register the migration in db.rs**

In `src-tauri/src/db.rs`, add to the `MIGRATIONS` array:

```rust
    Migration {
        name: "007_add_chat_threads",
        sql: include_str!("../migrations/007_add_chat_threads.sql"),
    },
```

Update the test `migrations_apply_on_fresh_db` assertion from `assert_eq!(count, 6)` to `assert_eq!(count, 7)` and `migrations_are_idempotent` similarly.

- [ ] **Step 3: Add ChatThread to model.rs**

In `src-tauri/src/session/model.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatThread {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 4: Update conversation_store.rs for thread-aware queries**

In `src-tauri/src/session/conversation_store.rs`, add thread CRUD functions:

```rust
use super::model::{ChatThread, ConversationMessage};

pub fn create_thread(conn: &Connection, session_id: &str, title: &str) -> AppResult<ChatThread> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO chat_threads (id, session_id, title) VALUES (?1, ?2, ?3)",
        params![id, session_id, title],
    )?;
    get_thread(conn, &id)
}

pub fn get_thread(conn: &Connection, thread_id: &str) -> AppResult<ChatThread> {
    conn.query_row(
        "SELECT id, session_id, title, created_at, updated_at FROM chat_threads WHERE id = ?1",
        params![thread_id],
        |row| Ok(ChatThread {
            id: row.get(0)?,
            session_id: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }),
    ).map_err(|e| crate::error::AppError::Db(e))
}

pub fn list_threads(conn: &Connection, session_id: &str) -> AppResult<Vec<ChatThread>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, title, created_at, updated_at
         FROM chat_threads WHERE session_id = ?1
         ORDER BY created_at ASC",
    )?;
    let threads = stmt
        .query_map(params![session_id], |row| Ok(ChatThread {
            id: row.get(0)?,
            session_id: row.get(1)?,
            title: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        }))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(threads)
}

pub fn rename_thread(conn: &Connection, thread_id: &str, title: &str) -> AppResult<ChatThread> {
    conn.execute(
        "UPDATE chat_threads SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![title, thread_id],
    )?;
    get_thread(conn, thread_id)
}

pub fn delete_thread(conn: &Connection, thread_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM chat_threads WHERE id = ?1", params![thread_id])?;
    Ok(())
}

pub fn get_or_create_default_thread(conn: &Connection, session_id: &str) -> AppResult<ChatThread> {
    let existing = list_threads(conn, session_id)?;
    if let Some(thread) = existing.first() {
        return Ok(thread.clone());
    }
    create_thread(conn, session_id, "Chat 1")
}
```

Update `save_message` to accept `thread_id`:

```rust
pub fn save_message(conn: &Connection, session_id: &str, thread_id: &str, role: &str, content: &str) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, thread_id, role, content) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_id, thread_id, role, content],
    )?;
    Ok(())
}
```

Update `list_by_session` to filter by thread:

```rust
pub fn list_by_thread(conn: &Connection, thread_id: &str) -> AppResult<Vec<ConversationMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at
         FROM conversations WHERE thread_id = ?1
         ORDER BY created_at ASC",
    )?;
    let messages = stmt
        .query_map(params![thread_id], row_to_message)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(messages)
}
```

Keep `list_by_session` as-is for backwards compatibility (search uses it).

Update `delete_from_message` to use thread_id:

```rust
pub fn delete_from_message(conn: &Connection, thread_id: &str, message_id: &str) -> AppResult<usize> {
    let pivot_rowid: i64 = conn
        .query_row(
            "SELECT rowid FROM conversations WHERE id = ?1 AND thread_id = ?2",
            params![message_id, thread_id],
            |row| row.get(0),
        )
        .map_err(|_| crate::error::AppError::Other(format!("Message {message_id} not found")))?;

    let deleted = conn.execute(
        "DELETE FROM conversations WHERE thread_id = ?1 AND rowid >= ?2",
        params![thread_id, pivot_rowid],
    )?;

    Ok(deleted)
}
```

- [ ] **Step 5: Add thread commands to commands.rs**

In `src-tauri/src/session/commands.rs`, add:

```rust
use super::model::{ChatThread, ConversationMessage, Note, Session};

#[tauri::command]
pub fn create_chat_thread(db: State<Database>, session_id: String, title: String) -> Result<ChatThread, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::create_thread(&conn, &session_id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_chat_threads(db: State<Database>, session_id: String) -> Result<Vec<ChatThread>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::list_threads(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_chat_thread(db: State<Database>, thread_id: String, title: String) -> Result<ChatThread, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::rename_thread(&conn, &thread_id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_chat_thread(db: State<Database>, thread_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::delete_thread(&conn, &thread_id).map_err(|e| e.to_string())
}
```

Update `get_conversation` to accept `thread_id`:

```rust
#[tauri::command]
pub fn get_conversation(
    db: State<Database>,
    thread_id: String,
) -> Result<Vec<ConversationMessage>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::list_by_thread(&conn, &thread_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_conversation_from(
    db: State<Database>,
    thread_id: String,
    message_id: String,
) -> Result<usize, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    super::conversation_store::delete_from_message(&conn, &thread_id, &message_id)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 6: Update streaming.rs for thread-aware message saving**

In `src-tauri/src/llm/streaming.rs`, update `send_message` to accept `thread_id`:

```rust
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    db: State<'_, Database>,
    session_id: String,
    thread_id: String,
    content: String,
    mode: String,
) -> Result<(), String> {
```

Update the `save_message` call to include `thread_id`:

```rust
        conversation_store::save_message(&conn, &session_id, &thread_id, "User", &content)
```

Update the conversation fetch to use thread_id:

```rust
        let mut conv_stmt = conn
            .prepare(
                "SELECT role, content FROM conversations WHERE thread_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let conversation: Vec<(String, String)> = conv_stmt
            .query_map(params![thread_id], |row| Ok((row.get(0)?, row.get(1)?)))
```

Update the assistant message save in the receiver spawn:

```rust
                if let Err(e) = conversation_store::save_message(
                    &conn,
                    &db_clone_session_id,
                    &db_clone_thread_id,
                    "Assistant",
                    &full_content,
                ) {
```

(Clone `thread_id` into `db_clone_thread_id` alongside `db_clone_session_id`)

- [ ] **Step 7: Register new commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the `invoke_handler`:

```rust
            create_chat_thread,
            list_chat_threads,
            rename_chat_thread,
            delete_chat_thread,
```

- [ ] **Step 8: Run tests**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck/src-tauri && cargo test 2>&1 | tail -20
```

Expected: existing tests may need updating for the new `thread_id` parameter in `save_message`. Update conversation_store tests to create a thread first and pass its ID.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/migrations/007_add_chat_threads.sql src-tauri/src/db.rs src-tauri/src/session/model.rs src-tauri/src/session/conversation_store.rs src-tauri/src/session/commands.rs src-tauri/src/lib.rs src-tauri/src/llm/streaming.rs
git commit -m "feat: add multi-chat per session with chat threads"
```

---

## Task 9: Multi-Chat Per Session — Frontend

**Files:**
- Modify: `src/features/chat/chat.types.ts`
- Modify: `src/features/chat/chat.atoms.ts`
- Modify: `src/features/session/session.atoms.ts`
- Modify: `src/features/session/SessionSidebar.tsx`
- Modify: `src/features/chat/ChatPanel.tsx`

- [ ] **Step 1: Add ChatThread type**

In `src/features/chat/chat.types.ts`, add:

```ts
export interface ChatThread {
  id: string;
  session_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}
```

- [ ] **Step 2: Add thread atoms**

In `src/features/chat/chat.atoms.ts`, add:

```ts
import type { ChatThread, ConversationMessage } from "./chat.types";

export const chatThreadsAtom = atom<ChatThread[]>([]);
export const activeThreadIdAtom = atom<string | null>(null);

export const activeThreadAtom = atom((get) => {
  const threads = get(chatThreadsAtom);
  const id = get(activeThreadIdAtom);
  return id ? (threads.find((t) => t.id === id) ?? null) : null;
});
```

- [ ] **Step 3: Update SessionSidebar to show threads**

In `src/features/session/SessionSidebar.tsx`, add imports:

```tsx
import { invoke } from "@tauri-apps/api/core";
import { chatThreadsAtom, activeThreadIdAtom } from "@/features/chat/chat.atoms";
import type { ChatThread } from "@/features/chat/chat.types";
import { MessageSquare, Plus } from "lucide-react";
```

Add thread state:

```tsx
  const [chatThreads, setChatThreads] = useAtom(chatThreadsAtom);
  const [activeThreadId, setActiveThreadId] = useAtom(activeThreadIdAtom);
```

When a session is clicked, load its threads:

```tsx
    async function handleSessionClick(sessionId: string) {
      setActiveId(sessionId);
      const threads = await invoke<ChatThread[]>("list_chat_threads", { sessionId });
      setChatThreads(threads);
      if (threads.length > 0) {
        setActiveThreadId(threads[threads.length - 1].id);
      } else {
        setActiveThreadId(null);
      }
    }
```

In the session list rendering, show threads under the active session:

```tsx
        {sessions.map((session) => (
          <div key={session.id}>
            <button
              onClick={() => handleSessionClick(session.id)}
              className={`mb-0.5 w-full rounded-md px-3 py-2 text-left text-sm transition-colors ${
                activeId === session.id
                  ? "bg-sidebar-accent text-sidebar-accent-foreground"
                  : "text-sidebar-foreground hover:bg-sidebar-accent/50"
              }`}
            >
              <span className="block truncate">{session.title}</span>
              <span className="block truncate text-xs text-muted-foreground">
                {new Date(session.updated_at + "Z").toLocaleDateString()}
              </span>
            </button>

            {activeId === session.id && chatThreads.length > 0 && (
              <div className="ml-4 mb-1 space-y-0.5">
                {chatThreads.map((thread) => (
                  <button
                    key={thread.id}
                    onClick={() => setActiveThreadId(thread.id)}
                    className={`w-full rounded px-2 py-1 text-left text-xs flex items-center gap-1.5 transition-colors ${
                      activeThreadId === thread.id
                        ? "bg-sidebar-accent/70 text-sidebar-accent-foreground"
                        : "text-muted-foreground hover:bg-sidebar-accent/40"
                    }`}
                  >
                    <MessageSquare className="size-3 shrink-0" />
                    <span className="truncate">{thread.title}</span>
                  </button>
                ))}
                <button
                  onClick={async () => {
                    const count = chatThreads.length + 1;
                    const thread = await invoke<ChatThread>("create_chat_thread", {
                      sessionId: session.id,
                      title: `Chat ${count}`,
                    });
                    setChatThreads([...chatThreads, thread]);
                    setActiveThreadId(thread.id);
                  }}
                  className="w-full rounded px-2 py-1 text-left text-xs flex items-center gap-1.5 text-muted-foreground/60 hover:text-muted-foreground hover:bg-sidebar-accent/40 transition-colors"
                >
                  <Plus className="size-3" />
                  New Chat
                </button>
              </div>
            )}
          </div>
        ))}
```

- [ ] **Step 4: Update ChatPanel to use thread_id**

In `src/features/chat/ChatPanel.tsx`, import the new atoms:

```tsx
import {
  activeThreadIdAtom,
  activeThreadAtom,
  chatThreadsAtom,
  chatModeAtom,
  conversationAtom,
  isExtractingAtom,
  isStreamingAtom,
  ragContextAtom,
  streamingContentAtom,
} from "./chat.atoms";
import type { ChatThread } from "./chat.types";
```

Add thread state:

```tsx
  const activeThread = useAtomValue(activeThreadAtom);
  const [activeThreadId, setActiveThreadId] = useAtom(activeThreadIdAtom);
  const [chatThreads, setChatThreads] = useAtom(chatThreadsAtom);
```

Update the conversation loading effect to use thread_id:

```tsx
  useEffect(() => {
    if (!activeThread) {
      setConversation([]);
      return;
    }
    invoke<ConversationMessage[]>("get_conversation", {
      threadId: activeThread.id,
    }).then(setConversation);
  }, [activeThread?.id]);
```

Update `handleSend` to pass `threadId`:

```tsx
  async function handleSend() {
    const text = inputValue.trim();
    if (!text || !activeSession || !activeThread || isStreaming) return;

    setInputValue("");
    setErrors([]);
    setIsStreaming(true);
    setStreamingContent("");
    setRagContext(null);
    shouldAutoScroll.current = true;

    await invoke("send_message", {
      sessionId: activeSession.id,
      threadId: activeThread.id,
      content: text,
      mode: chatMode,
    });
  }
```

Update `handleEditSubmit` similarly to pass `threadId`.

Update the `llm:done` handler to reload with `threadId`:

```tsx
      listen<{ full_content: string }>("llm:done", () => {
        if (isExtractingRef.current) return;
        setIsStreaming(false);
        setStreamingContent("");
        setRagContext(null);
        if (activeThread) {
          invoke<ConversationMessage[]>("get_conversation", {
            threadId: activeThread.id,
          }).then(setConversation);
        }
      }),
```

Add a "New Chat" button to the ChatPanel header:

```tsx
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <h2 className="text-sm font-medium text-muted-foreground truncate">
          {activeThread?.title ?? "Duck Chat"}
        </h2>
        <div className="ml-auto flex gap-1">
          <Button
            variant="ghost"
            size="xs"
            onClick={async () => {
              if (!activeSession) return;
              const count = chatThreads.length + 1;
              const thread = await invoke<ChatThread>("create_chat_thread", {
                sessionId: activeSession.id,
                title: `Chat ${count}`,
              });
              setChatThreads([...chatThreads, thread]);
              setActiveThreadId(thread.id);
            }}
            className="text-muted-foreground"
            title="New chat"
          >
            <Plus className="size-3.5" />
          </Button>
          {/* Existing Assist/Grill buttons */}
```

Update the empty state to also check for activeThread:

```tsx
  if (!activeSession || !activeThread) {
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center p-4">
        <p className="text-xs text-muted-foreground/60">
          Select a session to start chatting
        </p>
      </div>
    );
  }
```

- [ ] **Step 5: Auto-create thread on first session load**

In `SessionSidebar.tsx`, when a session is selected and it has no threads, auto-create one:

```tsx
    async function handleSessionClick(sessionId: string) {
      setActiveId(sessionId);
      let threads = await invoke<ChatThread[]>("list_chat_threads", { sessionId });
      if (threads.length === 0) {
        const thread = await invoke<ChatThread>("create_chat_thread", {
          sessionId,
          title: "Chat 1",
        });
        threads = [thread];
      }
      setChatThreads(threads);
      setActiveThreadId(threads[threads.length - 1].id);
    }
```

- [ ] **Step 6: Build and verify**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully.

- [ ] **Step 7: Commit**

```bash
git add src/features/chat/chat.types.ts src/features/chat/chat.atoms.ts src/features/session/SessionSidebar.tsx src/features/chat/ChatPanel.tsx
git commit -m "feat: multi-chat per session with thread sidebar and switching"
```

---

## Task 10: Global Search — Backend

**Files:**
- Create: `src-tauri/migrations/008_add_conversation_fts.sql`
- Create: `src-tauri/src/search/mod.rs`
- Create: `src-tauri/src/search/store.rs`
- Create: `src-tauri/src/search/commands.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write FTS migration for conversation content**

Create `src-tauri/migrations/008_add_conversation_fts.sql`:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
    content,
    conversation_id UNINDEXED,
    session_id UNINDEXED
);

-- Populate from existing data
INSERT INTO conversations_fts (content, conversation_id, session_id)
SELECT content, id, session_id FROM conversations;

-- Trigger to keep FTS in sync on insert
CREATE TRIGGER conversations_fts_insert AFTER INSERT ON conversations
BEGIN
    INSERT INTO conversations_fts (content, conversation_id, session_id)
    VALUES (NEW.content, NEW.id, NEW.session_id);
END;

-- Trigger to remove on delete
CREATE TRIGGER conversations_fts_delete AFTER DELETE ON conversations
BEGIN
    DELETE FROM conversations_fts WHERE conversation_id = OLD.id;
END;
```

- [ ] **Step 2: Register migration in db.rs**

In `src-tauri/src/db.rs`, add:

```rust
    Migration {
        name: "008_add_conversation_fts",
        sql: include_str!("../migrations/008_add_conversation_fts.sql"),
    },
```

Update migration count assertions to 8.

- [ ] **Step 3: Create search module**

Create `src-tauri/src/search/mod.rs`:

```rust
pub mod commands;
pub mod store;
```

Create `src-tauri/src/search/store.rs`:

```rust
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub content_type: String,
    pub session_id: String,
    pub session_name: String,
    pub thread_id: Option<String>,
    pub source_id: String,
    pub preview: String,
}

pub fn search_all(conn: &Connection, query: &str) -> AppResult<Vec<SearchResult>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();

    // Search conversations via FTS5
    let fts_query = format!("\"{}\"", query.replace('"', "\"\""));
    if let Ok(mut stmt) = conn.prepare(
        "SELECT snippet(conversations_fts, 0, '**', '**', '...', 40) as preview,
                cf.conversation_id, cf.session_id, s.title as session_name,
                c.thread_id
         FROM conversations_fts cf
         JOIN conversations c ON c.id = cf.conversation_id
         JOIN sessions s ON s.id = cf.session_id
         WHERE conversations_fts MATCH ?1
         ORDER BY rank
         LIMIT 20"
    ) {
        if let Ok(rows) = stmt.query_map(params![fts_query], |row| {
            Ok(SearchResult {
                content_type: "chat".to_string(),
                preview: row.get(0)?,
                source_id: row.get(1)?,
                session_id: row.get(2)?,
                session_name: row.get(3)?,
                thread_id: row.get(4)?,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    // Search notes
    if let Ok(mut stmt) = conn.prepare(
        "SELECT n.content, n.session_id, s.title as session_name, n.id
         FROM notes n
         JOIN sessions s ON s.id = n.session_id
         WHERE n.content LIKE ?1
         LIMIT 10"
    ) {
        let like_query = format!("%{query}%");
        if let Ok(rows) = stmt.query_map(params![like_query], |row| {
            let content: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let session_name: String = row.get(2)?;
            let source_id: String = row.get(3)?;

            // Extract a preview around the match
            let lower = content.to_lowercase();
            let q_lower = query.to_lowercase();
            let preview = if let Some(pos) = lower.find(&q_lower) {
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(content.len());
                let slice = &content[start..end];
                if start > 0 { format!("...{slice}...") } else { format!("{slice}...") }
            } else {
                content.chars().take(80).collect::<String>()
            };

            Ok(SearchResult {
                content_type: "note".to_string(),
                preview,
                source_id,
                session_id,
                session_name,
                thread_id: None,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    // Search documents
    if let Ok(mut stmt) = conn.prepare(
        "SELECT ds.content, d.session_id, s.title as session_name, ds.id, d.title
         FROM document_sections ds
         JOIN documents d ON d.id = ds.document_id
         JOIN sessions s ON s.id = d.session_id
         WHERE ds.content LIKE ?1
         LIMIT 10"
    ) {
        let like_query = format!("%{query}%");
        if let Ok(rows) = stmt.query_map(params![like_query], |row| {
            let content: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let session_name: String = row.get(2)?;
            let source_id: String = row.get(3)?;
            let _doc_title: String = row.get(4)?;

            let lower = content.to_lowercase();
            let q_lower = query.to_lowercase();
            let preview = if let Some(pos) = lower.find(&q_lower) {
                let start = pos.saturating_sub(40);
                let end = (pos + query.len() + 40).min(content.len());
                let slice = &content[start..end];
                if start > 0 { format!("...{slice}...") } else { format!("{slice}...") }
            } else {
                content.chars().take(80).collect::<String>()
            };

            Ok(SearchResult {
                content_type: "doc".to_string(),
                preview,
                source_id,
                session_id,
                session_name,
                thread_id: None,
            })
        }) {
            results.extend(rows.filter_map(|r| r.ok()));
        }
    }

    Ok(results)
}
```

Create `src-tauri/src/search/commands.rs`:

```rust
use tauri::State;

use crate::db::Database;
use super::store::{self, SearchResult};

#[tauri::command]
pub fn search_all(db: State<Database>, query: String) -> Result<Vec<SearchResult>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::search_all(&conn, &query).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Register search module in lib.rs**

In `src-tauri/src/lib.rs`, add:

```rust
mod search;
use search::commands::*;
```

In the `invoke_handler`, add:

```rust
            search_all,
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck/src-tauri && cargo test 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/migrations/008_add_conversation_fts.sql src-tauri/src/search/ src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat: add global search backend with FTS5 across chats, notes, docs"
```

---

## Task 11: Global Search — Frontend

**Files:**
- Create: `src/components/SearchPalette.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create SearchPalette component**

Create `src/components/SearchPalette.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { Command } from "cmdk";
import { FileText, MessageSquare, Notebook, Search } from "lucide-react";
import { activeSessionIdAtom } from "@/features/session/session.atoms";
import { activeThreadIdAtom, chatThreadsAtom } from "@/features/chat/chat.atoms";
import type { ChatThread } from "@/features/chat/chat.types";

interface SearchResult {
  content_type: string;
  session_id: string;
  session_name: string;
  thread_id: string | null;
  source_id: string;
  preview: string;
}

interface SearchPaletteProps {
  open: boolean;
  onClose: () => void;
  onNavigate?: (tab: string) => void;
}

export function SearchPalette({ open, onClose, onNavigate }: SearchPaletteProps) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const setActiveSessionId = useSetAtom(activeSessionIdAtom);
  const setActiveThreadId = useSetAtom(activeThreadIdAtom);
  const setChatThreads = useSetAtom(chatThreadsAtom);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setResults([]);
    }
  }, [open]);

  useEffect(() => {
    if (query.trim().length < 2) {
      setResults([]);
      return;
    }
    const timeout = setTimeout(async () => {
      const res = await invoke<SearchResult[]>("search_all", { query: query.trim() });
      setResults(res);
    }, 200);
    return () => clearTimeout(timeout);
  }, [query]);

  async function handleSelect(result: SearchResult) {
    setActiveSessionId(result.session_id);

    if (result.content_type === "chat" && result.thread_id) {
      const threads = await invoke<ChatThread[]>("list_chat_threads", { sessionId: result.session_id });
      setChatThreads(threads);
      setActiveThreadId(result.thread_id);
    } else if (result.content_type === "note") {
      onNavigate?.("dump");
    } else if (result.content_type === "doc") {
      onNavigate?.("docs");
    }

    onClose();
  }

  if (!open) return null;

  const chatResults = results.filter((r) => r.content_type === "chat");
  const noteResults = results.filter((r) => r.content_type === "note");
  const docResults = results.filter((r) => r.content_type === "doc");

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]" onClick={onClose}>
      <div className="absolute inset-0 bg-black/50" />
      <div
        className="relative w-full max-w-lg rounded-lg border border-border bg-popover shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <Command shouldFilter={false}>
          <div className="flex items-center gap-2 border-b border-border px-3">
            <Search className="size-4 text-muted-foreground" />
            <Command.Input
              value={query}
              onValueChange={setQuery}
              placeholder="Search chats, notes, docs..."
              className="flex-1 bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === "Escape") onClose();
              }}
            />
          </div>

          <Command.List className="max-h-72 overflow-y-auto p-1">
            {query.trim().length >= 2 && results.length === 0 && (
              <Command.Empty className="py-6 text-center text-sm text-muted-foreground">
                No results found
              </Command.Empty>
            )}

            {chatResults.length > 0 && (
              <Command.Group heading="Chats">
                {chatResults.map((r) => (
                  <Command.Item
                    key={`chat-${r.source_id}`}
                    onSelect={() => handleSelect(r)}
                    className="flex items-start gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer aria-selected:bg-accent"
                  >
                    <MessageSquare className="size-4 shrink-0 mt-0.5 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs text-muted-foreground">{r.session_name}</p>
                      <p className="text-sm">{r.preview}</p>
                    </div>
                  </Command.Item>
                ))}
              </Command.Group>
            )}

            {noteResults.length > 0 && (
              <Command.Group heading="Notes">
                {noteResults.map((r) => (
                  <Command.Item
                    key={`note-${r.source_id}`}
                    onSelect={() => handleSelect(r)}
                    className="flex items-start gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer aria-selected:bg-accent"
                  >
                    <Notebook className="size-4 shrink-0 mt-0.5 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs text-muted-foreground">{r.session_name}</p>
                      <p className="text-sm">{r.preview}</p>
                    </div>
                  </Command.Item>
                ))}
              </Command.Group>
            )}

            {docResults.length > 0 && (
              <Command.Group heading="Docs">
                {docResults.map((r) => (
                  <Command.Item
                    key={`doc-${r.source_id}`}
                    onSelect={() => handleSelect(r)}
                    className="flex items-start gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer aria-selected:bg-accent"
                  >
                    <FileText className="size-4 shrink-0 mt-0.5 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-xs text-muted-foreground">{r.session_name}</p>
                      <p className="text-sm">{r.preview}</p>
                    </div>
                  </Command.Item>
                ))}
              </Command.Group>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Wire Cmd+K in App.tsx**

In `src/App.tsx`, add imports:

```tsx
import { SearchPalette } from "@/components/SearchPalette";
```

Add state for search palette:

```tsx
  const [searchOpen, setSearchOpen] = useState(false);
```

Add keyboard listener in a useEffect:

```tsx
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setSearchOpen((prev) => !prev);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
```

Render the SearchPalette before the closing `</div>`, passing the tab setter:

```tsx
      <SearchPalette
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        onNavigate={(tab) => setActiveTab(tab as Tab)}
      />
      <SettingsDialog />
    </div>
```

- [ ] **Step 3: Build and verify**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src/components/SearchPalette.tsx src/App.tsx
git commit -m "feat: add Cmd+K global search palette across chats, notes, docs"
```

---

## Task 12: RAG Visibility + Smarter Search

**Files:**
- Create: `src-tauri/migrations/009_add_rag_context.sql`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/llm/streaming.rs`
- Modify: `src-tauri/src/session/conversation_store.rs`
- Modify: `src-tauri/src/rag/search.rs`
- Modify: `src/features/chat/ChatPanel.tsx`
- Modify: `src/features/chat/chat.types.ts`

- [ ] **Step 1: Write rag_context migration**

Create `src-tauri/migrations/009_add_rag_context.sql`:

```sql
ALTER TABLE conversations ADD COLUMN rag_context TEXT;
```

Register in `src-tauri/src/db.rs`:

```rust
    Migration {
        name: "009_add_rag_context",
        sql: include_str!("../migrations/009_add_rag_context.sql"),
    },
```

Update migration count assertions to 9.

- [ ] **Step 2: Update conversation_store to save/return rag_context**

In `src-tauri/src/session/conversation_store.rs`, update `save_message` to accept optional rag_context:

```rust
pub fn save_message_with_context(
    conn: &Connection,
    session_id: &str,
    thread_id: &str,
    role: &str,
    content: &str,
    rag_context: Option<&str>,
) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO conversations (id, session_id, thread_id, role, content, rag_context) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, session_id, thread_id, role, content, rag_context],
    )?;
    Ok(())
}
```

Update `row_to_message` and `ConversationMessage` to include `rag_context`:

In `model.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub rag_context: Option<String>,
}
```

In `conversation_store.rs`, update `row_to_message`:

```rust
fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<ConversationMessage> {
    Ok(ConversationMessage {
        id: row.get(0)?,
        role: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        rag_context: row.get(4)?,
    })
}
```

Update the SELECT in `list_by_thread`:

```rust
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at, rag_context
         FROM conversations WHERE thread_id = ?1
         ORDER BY created_at ASC",
    )?;
```

- [ ] **Step 3: Store RAG context when saving assistant message in streaming.rs**

In `src-tauri/src/llm/streaming.rs`, after the RAG search, serialize the retrieved chunks into JSON. Before the streaming spawns, create a `rag_context_json`:

```rust
    let rag_context_json: Option<String> = if !retrieved_chunks.is_empty() {
        serde_json::to_string(&retrieved_chunks.iter().map(|c| {
            serde_json::json!({
                "file_path": c.file_path,
                "repo_name": c.repo_name,
                "start_line": c.start_line,
                "end_line": c.end_line,
            })
        }).collect::<Vec<_>>()).ok()
    } else {
        None
    };
```

Clone it into the receiver spawn and use `save_message_with_context` instead of `save_message`:

```rust
    let rag_json_clone = rag_context_json.clone();
    // In the receiver spawn:
                if let Err(e) = conversation_store::save_message_with_context(
                    &conn,
                    &db_clone_session_id,
                    &db_clone_thread_id,
                    "Assistant",
                    &full_content,
                    rag_json_clone.as_deref(),
                ) {
```

- [ ] **Step 4: Add query expansion to rag/search.rs**

In `src-tauri/src/rag/search.rs`, add a function to extract search terms:

```rust
pub fn extract_search_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();

    // CamelCase identifiers
    let camel_re = regex::Regex::new(r"\b[A-Z][a-z]+(?:[A-Z][a-z]+)+\b").unwrap();
    for m in camel_re.find_iter(text) {
        terms.push(m.as_str().to_string());
    }

    // snake_case identifiers
    let snake_re = regex::Regex::new(r"\b[a-z]+(?:_[a-z]+)+\b").unwrap();
    for m in snake_re.find_iter(text) {
        terms.push(m.as_str().to_string());
    }

    // Quoted strings
    let quote_re = regex::Regex::new(r#""([^"]+)""#).unwrap();
    for cap in quote_re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            terms.push(m.as_str().to_string());
        }
    }

    // @mentions (strip the @)
    let mention_re = regex::Regex::new(r"@([\w.\-]+/[\w.\-/]+)").unwrap();
    for cap in mention_re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            terms.push(m.as_str().to_string());
        }
    }

    terms.dedup();
    terms
}

pub fn build_expanded_fts_query(user_text: &str, recent_messages: &[(String, String)]) -> String {
    let mut all_text = user_text.to_string();
    // Include last 2-3 messages for conversational context
    for (_role, content) in recent_messages.iter().rev().take(3) {
        all_text.push(' ');
        all_text.push_str(content);
    }

    let terms = extract_search_terms(&all_text);
    let words: Vec<&str> = all_text.split_whitespace()
        .filter(|w| w.len() > 3)
        .take(10)
        .collect();

    let mut fts_parts: Vec<String> = terms.iter().map(|t| format!("\"{t}\"")).collect();
    for w in words {
        let cleaned = w.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.len() > 3 && !fts_parts.iter().any(|p| p.contains(cleaned)) {
            fts_parts.push(format!("\"{cleaned}\""));
        }
    }

    if fts_parts.is_empty() {
        return format!("\"{}\"", user_text.replace('"', "\"\""));
    }

    fts_parts.join(" OR ")
}

pub fn adaptive_top_k(search_terms: &[String]) -> usize {
    if search_terms.len() > 3 {
        15
    } else {
        TOP_K
    }
}
```

Update `hybrid_search` to accept an expanded FTS query and adaptive K:

```rust
pub fn hybrid_search(
    conn: &Connection,
    query_embedding: &[f32],
    query_text: &str,
    repo_ids: &[String],
) -> AppResult<Vec<RetrievedChunk>> {
```

Change the function to use `build_expanded_fts_query` internally for the FTS query. Replace the raw `query_text` in the FTS query with an expanded version:

```rust
    let expanded_query = build_expanded_fts_query(query_text, &[]);
    // ... use expanded_query instead of query_text in fts_params
```

And change `TOP_K` usage to `adaptive_top_k(&extract_search_terms(query_text))`.

- [ ] **Step 5: Add RAG context display in ChatPanel**

In `src/features/chat/chat.types.ts`, add:

```ts
export interface RagChunkInfo {
  file_path: string;
  repo_name: string;
  start_line: number;
  end_line: number;
}
```

Update the `ConversationMessage` interface:

```ts
export interface ConversationMessage {
  id: string;
  role: "User" | "Assistant" | "System";
  content: string;
  created_at: string;
  rag_context: string | null;
}
```

In `ChatPanel.tsx`, after the assistant message markdown content, add a collapsible context section:

```tsx
                  {msg.role === "Assistant" && msg.rag_context && (() => {
                    const chunks: RagChunkInfo[] = JSON.parse(msg.rag_context);
                    const repos = new Set(chunks.map(c => c.repo_name));
                    return (
                      <details className="mt-1 text-[11px] text-muted-foreground/70">
                        <summary className="cursor-pointer hover:text-muted-foreground">
                          Used {chunks.length} file{chunks.length !== 1 ? "s" : ""} from {repos.size} repo{repos.size !== 1 ? "s" : ""}
                        </summary>
                        <div className="mt-1 space-y-0.5 pl-2">
                          {chunks.map((c, i) => (
                            <div key={i} className="font-mono">
                              {c.repo_name}/{c.file_path} L{c.start_line}-{c.end_line}
                            </div>
                          ))}
                        </div>
                      </details>
                    );
                  })()}
```

Import `RagChunkInfo` in ChatPanel:

```tsx
import type { ChatThread, ConversationMessage, RagChunkInfo } from "./chat.types";
```

- [ ] **Step 6: Run all tests**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck/src-tauri && cargo test 2>&1 | tail -20
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -5
```

Expected: all pass, frontend builds.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/migrations/009_add_rag_context.sql src-tauri/src/db.rs src-tauri/src/llm/streaming.rs src-tauri/src/session/conversation_store.rs src-tauri/src/session/model.rs src-tauri/src/rag/search.rs src/features/chat/ChatPanel.tsx src/features/chat/chat.types.ts
git commit -m "feat: add RAG context visibility and smarter query expansion"
```

---

## Verification Checklist

After all tasks are complete, verify each feature end-to-end:

- [ ] **Chat input**: Type multi-line message, verify auto-expand up to ~40vh, Enter sends, Shift+Enter newlines
- [ ] **Stop generation**: Start a generation, click Stop, verify partial response saved and input re-enabled
- [ ] **Markdown**: Send a message that triggers a code block response, verify syntax highlighting and copy button
- [ ] **Message copy**: Hover over assistant message, verify copy button appears and works
- [ ] **@ mention**: Type @, verify dropdown appears near cursor as portal, file paths readable, selection works
- [ ] **Ticket links**: Type a Jira ticket key (e.g. PROJ-123), verify it renders as clickable link
- [ ] **Multi-chat**: Create new chat in sidebar, switch between chats, verify messages are separate
- [ ] **Cmd+K search**: Press Cmd+K, type a query, verify results from chats/notes/docs appear
- [ ] **RAG context**: Send a message with repos attached, verify "Used N files" appears under assistant response
- [ ] **Text selection**: Verify all message text is selectable (no select-none blocking it)
