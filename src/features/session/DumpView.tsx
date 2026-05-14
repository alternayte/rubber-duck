import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import { MarkdownEditor } from "@/components/MarkdownEditor";

interface Note {
  id: string;
  session_id: string;
  content: string;
  sort_order: number;
  created_at: string;
}

type ViewMode = "edit" | "preview";

interface DumpViewProps {
  sessionId: string;
}

const AUTOSAVE_DELAY_MS = 500;

export function DumpView({ sessionId }: DumpViewProps) {
  const [note, setNote] = useState<Note | null>(null);
  const [content, setContent] = useState("");
  const [mode, setMode] = useState<ViewMode>("edit");
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const noteIdRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      const n = await invoke<Note>("get_or_create_note", {
        sessionId,
      });
      if (cancelled) return;
      setNote(n);
      setContent(n.content);
      noteIdRef.current = n.id;
    }

    load();
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  const saveContent = useCallback(
    async (newContent: string) => {
      const id = noteIdRef.current;
      if (!id) return;
      await invoke<Note>("update_note", { id, content: newContent });
    },
    [],
  );

  const handleChange = useCallback(
    (newContent: string) => {
      setContent(newContent);

      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
      }
      saveTimerRef.current = setTimeout(() => {
        saveContent(newContent);
      }, AUTOSAVE_DELAY_MS);
    },
    [saveContent],
  );

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
      }
    };
  }, []);

  if (!note) {
    return (
      <p className="text-sm text-muted-foreground">Loading...</p>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-1 pb-3">
        <Button
          variant={mode === "edit" ? "secondary" : "ghost"}
          size="xs"
          onClick={() => setMode("edit")}
        >
          Edit
        </Button>
        <Button
          variant={mode === "preview" ? "secondary" : "ghost"}
          size="xs"
          onClick={() => setMode("preview")}
        >
          Preview
        </Button>
      </div>

      <div className="min-h-0 flex-1">
        {mode === "edit" ? (
          <MarkdownEditor
            value={content}
            onChange={handleChange}
            placeholder="Brain dump here... markdown supported"
          />
        ) : (
          <div className="prose prose-invert prose-sm max-w-none overflow-auto h-full">
            {content ? (
              <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
            ) : (
              <p className="text-muted-foreground">Nothing here yet</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
