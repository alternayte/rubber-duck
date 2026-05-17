import { useCallback, useEffect, useRef, useState } from "react";
import type React from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useAtom, useAtomValue } from "jotai";
import { Button } from "@/components/ui/button";
import { MarkdownEditor } from "@/components/MarkdownEditor";
import { TicketList } from "@/features/ticket/TicketList";
import { apiKeySetAtom } from "@/features/settings/settings.atoms";
import { chatModeAtom, isExtractingAtom, isStreamingAtom } from "@/features/chat/chat.atoms";
import { useTicketActions } from "@/features/ticket/useTicketActions";
import { parseTicketsFromResponse } from "@/features/ticket/extractTickets";
import { JiraLinkedText } from "@/components/JiraLinkedText";

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

function processChildren(children: React.ReactNode): React.ReactNode {
  return Array.isArray(children)
    ? children.map((child, i) =>
        typeof child === "string" ? <JiraLinkedText key={i}>{child}</JiraLinkedText> : child,
      )
    : typeof children === "string"
      ? <JiraLinkedText>{children}</JiraLinkedText>
      : children;
}

export function DumpView({ sessionId }: DumpViewProps) {
  const [note, setNote] = useState<Note | null>(null);
  const [content, setContent] = useState("");
  const [mode, setMode] = useState<ViewMode>("edit");
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const noteIdRef = useRef<string | null>(null);

  const apiKeySet = useAtomValue(apiKeySetAtom);
  const isStreaming = useAtomValue(isStreamingAtom);
  const chatMode = useAtomValue(chatModeAtom);
  const { createTicket } = useTicketActions();
  const [extracting, setExtracting] = useAtom(isExtractingAtom);
  const [extractError, setExtractError] = useState<string | null>(null);

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

  async function handleImagePaste(base64: string): Promise<string | null> {
    try {
      const filePath = await invoke<string>("save_pasted_image", {
        sessionId,
        base64Data: base64,
      });
      const assetUrl = convertFileSrc(filePath, "rdimg");
      return `\n![pasted image](${assetUrl})\n`;
    } catch (e) {
      console.error("Failed to save image:", e);
      return null;
    }
  }

  async function handleExtractTickets() {
    if (!apiKeySet || isStreaming || extracting) return;
    setExtracting(true);
    setExtractError(null);

    // Listen for the done event to parse tickets from the response
    const unlisten = await listen<{ full_content: string }>("llm:done", async (event) => {
      const { tickets, error } = parseTicketsFromResponse(event.payload.full_content, sessionId);
      if (error) {
        setExtractError(error);
      } else {
        for (const params of tickets) {
          await createTicket(params);
        }
      }
      setExtracting(false);
      unlisten();
    });

    // Also listen for errors
    const unlistenError = await listen<{ message: string }>("llm:error", () => {
      setExtracting(false);
      setExtractError("LLM request failed — try again");
      unlisten();
      unlistenError();
    });

    const extractPrompt = `Read my brain dump notes carefully and extract structured work tickets from them.

Return the tickets as a JSON array inside a \`\`\`json code block. Each ticket object should have these fields:
- "title": short descriptive title (required)
- "description": detailed description of the work
- "acceptance_criteria": what "done" looks like
- "priority": one of "Low", "Medium", "High", "Critical"
- "ticket_type": one of "Task", "Bug", "Story", "Epic"
- "estimate": one of "XS", "S", "M", "L", "XL"

Only extract tickets that are clearly implied by the notes. Don't invent work that isn't there.`;

    await invoke("send_message", {
      sessionId,
      content: extractPrompt,
      mode: chatMode,
    });
  }

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

        <div className="ml-auto flex items-center gap-2">
          {extractError && (
            <span className="text-xs text-destructive-foreground">{extractError}</span>
          )}
          <Button
            variant="outline"
            size="xs"
            onClick={handleExtractTickets}
            disabled={!apiKeySet || isStreaming || extracting}
          >
            {extracting ? "Extracting..." : "Extract Tickets"}
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-[2] overflow-hidden">
        {mode === "edit" ? (
          <MarkdownEditor
            value={content}
            onChange={handleChange}
            onImagePaste={handleImagePaste}
            placeholder="Brain dump here... markdown supported"
            sessionId={sessionId}
          />
        ) : (
          <div className="prose prose-invert prose-sm max-w-none overflow-auto h-full">
            {content ? (
              <Markdown
                remarkPlugins={[remarkGfm]}
                components={{
                  p: ({ children }) => <p>{processChildren(children)}</p>,
                  li: ({ children }) => <li>{processChildren(children)}</li>,
                }}
              >{content}</Markdown>
            ) : (
              <p className="text-muted-foreground">Nothing here yet</p>
            )}
          </div>
        )}
      </div>

      <TicketList sessionId={sessionId} />
    </div>
  );
}
