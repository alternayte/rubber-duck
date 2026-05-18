import { useCallback, useEffect, useRef, useState } from "react";
import type React from "react";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Copy, Pencil, Plus, Square } from "lucide-react";
import { CodeBlock } from "@/components/CodeBlock";
import { Button } from "@/components/ui/button";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { apiKeySetAtom, settingsOpenAtom } from "@/features/settings/settings.atoms";
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
import type { ChatThread, ConversationMessage, RagChunkInfo } from "./chat.types";
import { JiraLinkedText } from "@/components/JiraLinkedText";
import { MentionText } from "@/components/MentionText";
import { AtMentionInput } from "@/components/AtMentionInput";

interface ErrorMessage {
  id: string;
  message: string;
}

function processChildren(children: React.ReactNode): React.ReactNode {
  if (Array.isArray(children)) {
    return children.map((child, i) =>
      typeof child === "string" ? <LinkedText key={i}>{child}</LinkedText> : child,
    );
  }
  return typeof children === "string" ? <LinkedText>{children}</LinkedText> : children;
}

function LinkedText({ children }: { children: string }) {
  const MENTION_SPLIT = /(@[\w.\-]+\/[\w.\-/]+)/g;
  const MENTION_TEST = /^@[\w.\-]+\/[\w.\-/]+$/;

  const parts = children.split(MENTION_SPLIT);
  return (
    <>
      {parts.map((part, i) =>
        MENTION_TEST.test(part) ? (
          <MentionText key={i}>{part}</MentionText>
        ) : (
          <JiraLinkedText key={i}>{part}</JiraLinkedText>
        ),
      )}
    </>
  );
}

function markdownComponents(processChildrenFn: typeof processChildren) {
  return {
    p: ({ children }: { children?: React.ReactNode }) => <p>{processChildrenFn(children)}</p>,
    li: ({ children }: { children?: React.ReactNode }) => <li>{processChildrenFn(children)}</li>,
    strong: ({ children }: { children?: React.ReactNode }) => <strong>{processChildrenFn(children)}</strong>,
    em: ({ children }: { children?: React.ReactNode }) => <em>{processChildrenFn(children)}</em>,
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

export function ChatPanel() {
  const activeSession = useAtomValue(activeSessionAtom);
  const apiKeySet = useAtomValue(apiKeySetAtom);
  const setSettingsOpen = useSetAtom(settingsOpenAtom);
  const [chatMode, setChatMode] = useAtom(chatModeAtom);
  const [conversation, setConversation] = useAtom(conversationAtom);
  const [isStreaming, setIsStreaming] = useAtom(isStreamingAtom);
  const [streamingContent, setStreamingContent] = useAtom(streamingContentAtom);
  const [ragContext, setRagContext] = useAtom(ragContextAtom);
  const [errors, setErrors] = useState<ErrorMessage[]>([]);
  const [inputValue, setInputValue] = useState("");
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const shouldAutoScroll = useRef(true);
  const listRef = useRef<HTMLDivElement>(null);
  const isExtractingRef = useRef(false);
  const isExtracting = useAtomValue(isExtractingAtom);
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState("");

  const activeThread = useAtomValue(activeThreadAtom);
  const [, setActiveThreadId] = useAtom(activeThreadIdAtom);
  const [chatThreads, setChatThreads] = useAtom(chatThreadsAtom);

  useEffect(() => {
    isExtractingRef.current = isExtracting;
  }, [isExtracting]);

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
    if (!activeThread) {
      setConversation([]);
      return;
    }
    invoke<ConversationMessage[]>("get_conversation", {
      threadId: activeThread.id,
    }).then(setConversation);
  }, [activeThread?.id]);

  useEffect(() => {
    const promises = [
      listen<{ content: string }>("llm:chunk", (event) => {
        setStreamingContent((prev) => prev + event.payload.content);
      }),

      listen<{ file_count: number; repo_count: number }>("rag:context", (event) => {
        setRagContext({
          fileCount: event.payload.file_count,
          repoCount: event.payload.repo_count,
        });
      }),

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
      }),
    ];

    return () => {
      promises.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, [activeThread?.id]);

  useEffect(() => {
    scrollToBottom();
  }, [conversation, streamingContent, errors]);

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

  async function handleEditSubmit() {
    const text = editingContent.trim();
    if (!text || !activeSession || !activeThread || isStreaming) return;

    const messageId = editingMessageId;
    setEditingMessageId(null);
    setEditingContent("");
    setErrors([]);
    setIsStreaming(true);
    setStreamingContent("");
    setRagContext(null);
    shouldAutoScroll.current = true;

    await invoke("delete_conversation_from", {
      threadId: activeThread.id,
      messageId,
    });

    const updated = await invoke<ConversationMessage[]>("get_conversation", {
      threadId: activeThread.id,
    });
    setConversation(updated);

    await invoke("send_message", {
      sessionId: activeSession.id,
      threadId: activeThread.id,
      content: text,
      mode: chatMode,
    });
  }

  async function handleCancel() {
    if (!activeSession) return;
    await invoke("cancel_generation", { sessionId: activeSession.id });
  }

  if (!activeSession || !activeThread) {
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
                <div className="relative group/msg">
                  <button
                    onClick={() => navigator.clipboard.writeText(msg.content)}
                    className="absolute top-0 right-0 hidden group-hover/msg:flex items-center gap-1 rounded bg-accent px-1.5 py-0.5 text-xs text-muted-foreground hover:text-foreground z-10"
                  >
                    <Copy className="size-3" /> Copy
                  </button>
                  <div className="prose prose-invert prose-sm max-w-none">
                    <Markdown
                      remarkPlugins={[remarkGfm]}
                      components={markdownComponents(processChildren)}
                    >{msg.content}</Markdown>
                  </div>
                  {msg.role === "Assistant" && msg.rag_context && (() => {
                    const chunks: RagChunkInfo[] = JSON.parse(msg.rag_context);
                    const repos = new Set(chunks.map((c: RagChunkInfo) => c.repo_name));
                    return (
                      <details className="mt-1 text-[11px] text-muted-foreground/70">
                        <summary className="cursor-pointer hover:text-muted-foreground">
                          Used {chunks.length} file{chunks.length !== 1 ? "s" : ""} from {repos.size} repo{repos.size !== 1 ? "s" : ""}
                        </summary>
                        <div className="mt-1 space-y-0.5 pl-2">
                          {chunks.map((c: RagChunkInfo, i: number) => (
                            <div key={i} className="font-mono">
                              {c.repo_name}/{c.file_path} L{c.start_line}-{c.end_line}
                            </div>
                          ))}
                        </div>
                      </details>
                    );
                  })()}
                </div>
              )}
            </div>
          );
        })}

        {isStreaming && ragContext && (
          <div className="mr-4 text-[11px] text-muted-foreground/70 py-1">
            Drawing from {ragContext.fileCount} file{ragContext.fileCount !== 1 ? "s" : ""} across{" "}
            {ragContext.repoCount} repo{ragContext.repoCount !== 1 ? "s" : ""}
          </div>
        )}

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
        <div className="flex gap-2">
          <AtMentionInput
            value={inputValue}
            onChange={setInputValue}
            onSubmit={handleSend}
            sessionId={activeSession.id}
            placeholder={
              chatMode === "grill"
                ? "Ask the duck to grill your plan..."
                : "Ask the duck..."
            }
            disabled={isStreaming || editingMessageId != null}
          />
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
        </div>
      </div>
    </div>
  );
}
