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
  isExtractingAtom,
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
  const isExtractingRef = useRef(false);
  const isExtracting = useAtomValue(isExtractingAtom);

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
      if (isExtractingRef.current) return;
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
