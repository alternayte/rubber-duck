import { useEffect, useRef, useState } from "react";
import { useAtom, useAtomValue, useSetAtom } from "jotai";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { sessionsAtom, activeSessionIdAtom } from "./session.atoms";
import { useSessionActions } from "./useSessionActions";
import { settingsOpenAtom } from "@/features/settings/settings.atoms";
import { Settings } from "lucide-react";

export function SessionSidebar() {
  const sessions = useAtomValue(sessionsAtom);
  const [activeId, setActiveId] = useAtom(activeSessionIdAtom);
  const { loadSessions, createSession } = useSessionActions();
  const setSettingsOpen = useSetAtom(settingsOpenAtom);
  const [isCreating, setIsCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

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
          <button
            key={session.id}
            onClick={() => setActiveId(session.id)}
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
