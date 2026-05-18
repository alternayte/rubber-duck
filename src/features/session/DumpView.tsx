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
import { MentionText } from "@/components/MentionText";
import { CodeBlock } from "@/components/CodeBlock";

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

    const extractPrompt = `Read my brain dump notes carefully and extract structured kanban work tickets from them.

Return the tickets as a JSON array inside a \`\`\`json code block. Each ticket object should have these fields:
- "title": concise action-oriented title starting with a verb (e.g. "Add auth retry logic", "Fix SSO timeout") (required)
- "description": structured description using this format:
  **Context:** Why this work exists — the problem or need (1-2 sentences).
  **Scope:** What specifically to build or change. Reference files, APIs, or components if known from the notes.
  **Approach:** How to implement it (high-level technical approach, not step-by-step). If an AI coding agent will implement this, write it so the agent has enough context to start without asking clarifying questions.
  **Out of Scope:** What this ticket explicitly does NOT cover (prevents scope creep).
- "acceptance_criteria": verifiable checklist. Each item should be objectively testable — either by a human or an automated check. Prefix agent-verifiable items with "[auto]" (e.g. "[auto] All existing tests pass", "[auto] No TypeScript errors"). Prefix human-verified items with "[human]" (e.g. "[human] UX feels responsive on slow connections").
- "priority": one of "Low", "Medium", "High", "Critical"
- "ticket_type": one of "Task", "Bug", "Story", "Epic"
- "estimate": one of "XS", "S", "M", "L", "XL"

Only extract tickets that are clearly implied by the notes. Don't invent work that isn't there. Write tickets assuming they may be picked up by an AI coding agent — provide enough context and specificity that implementation can begin without ambiguity.`;

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
                  img: ({ src, alt }) => (
                    <img src={src} alt={alt || ""} className="max-w-full rounded-md my-2" />
                  ),
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
