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
