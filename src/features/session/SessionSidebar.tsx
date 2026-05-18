import { useEffect, useRef, useState } from "react";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { sessionsAtom, activeSessionIdAtom } from "./session.atoms";
import { useSessionActions } from "./useSessionActions";
import { settingsOpenAtom } from "@/features/settings/settings.atoms";
import { chatThreadsAtom, activeThreadIdAtom } from "@/features/chat/chat.atoms";
import type { ChatThread } from "@/features/chat/chat.types";
import { MessageSquare, Plus, Settings } from "lucide-react";

export function SessionSidebar() {
  const sessions = useAtomValue(sessionsAtom);
  const [activeId, setActiveId] = useAtom(activeSessionIdAtom);
  const { loadSessions, createSession } = useSessionActions();
  const setSettingsOpen = useSetAtom(settingsOpenAtom);
  const [isCreating, setIsCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const [chatThreads, setChatThreads] = useAtom(chatThreadsAtom);
  const [activeThreadId, setActiveThreadId] = useAtom(activeThreadIdAtom);

  useEffect(() => {
    loadSessions();
  }, []);

  useEffect(() => {
    if (isCreating) {
      inputRef.current?.focus();
    }
  }, [isCreating]);

  async function handleCreate() {
    const title = newTitle.trim();
    if (!title) return;
    await createSession(title);
    setNewTitle("");
    setIsCreating(false);
  }

  function handleCancel() {
    setNewTitle("");
    setIsCreating(false);
  }

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

  return (
    <aside className="flex w-56 flex-col border-r border-border bg-sidebar-background">
      <div className="border-b border-border p-4">
        <h1 className="text-lg font-semibold tracking-tight text-foreground">
          rubber-duck
        </h1>
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        {isCreating && (
          <div className="mb-2 px-1">
            <Input
              ref={inputRef}
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleCreate();
                if (e.key === "Escape") handleCancel();
              }}
              placeholder="Session title..."
              className="h-8 text-sm"
            />
          </div>
        )}

        {sessions.length === 0 && !isCreating && (
          <p className="px-2 py-8 text-center text-xs text-muted-foreground">
            No sessions yet
          </p>
        )}

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
      </div>

      <div className="border-t border-border p-2 space-y-1">
        <Button
          variant="secondary"
          size="sm"
          className="w-full"
          onClick={() => setIsCreating(true)}
        >
          + New Session
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="w-full text-muted-foreground"
          onClick={() => setSettingsOpen(true)}
        >
          <Settings className="size-4" />
          Settings
        </Button>
      </div>
    </aside>
  );
}
