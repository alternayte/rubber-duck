import { useEffect, useState } from "react";
import type React from "react";
import { useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Download,
  History,
  Pencil,
  RefreshCw,
  Square,
  Trash2,
} from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { Button } from "@/components/ui/button";
import { CodeBlock } from "@/components/CodeBlock";
import { sectionsByDocAtom, sectionGenerationAtom } from "./docs.atoms";
import { VersionHistory } from "./VersionHistory";
import { useDocActions } from "./useDocActions";
import type { Document, DocumentSection, SectionGenerationState } from "./docs.types";

interface DocumentCardProps {
  document: Document;
  sessionId: string;
  onDeleted: () => void;
}

export function DocumentCard({ document, sessionId, onDeleted }: DocumentCardProps) {
  const [expanded, setExpanded] = useState(true);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const sectionsByDoc = useAtomValue(sectionsByDocAtom);
  const sectionGeneration = useAtomValue(sectionGenerationAtom);
  const sections = sectionsByDoc[document.id] ?? [];

  const actions = useDocActions(sessionId);

  useEffect(() => {
    actions.loadSections(document.id);
  }, [document.id]);

  async function handleDelete() {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await actions.deleteDocument(document.id);
    onDeleted();
  }

  function buildMarkdown(): string {
    const parts = [`# ${document.title}\n`];
    for (const section of sections) {
      parts.push(`## ${section.name}\n\n${section.content}`);
    }
    return parts.join("\n\n");
  }

  async function handleExport() {
    const content = buildMarkdown();
    const filePath = await save({
      defaultPath: `${document.title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await writeTextFile(filePath, content);
    }
  }

  async function handleCopyAll() {
    await navigator.clipboard.writeText(buildMarkdown());
  }

  return (
    <div className="rounded-lg border border-border bg-card">
      {/* Card header */}
      <div className="flex items-center gap-2 px-4 py-3">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          {expanded ? (
            <ChevronDown className="size-4 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
          )}
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">{document.title}</p>
            <p className="text-xs text-muted-foreground">{document.template_name}</p>
          </div>
        </button>

        {confirmDelete ? (
          <div className="flex gap-1">
            <Button size="xs" variant="destructive" onClick={handleDelete}>
              Confirm
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setConfirmDelete(false)}>
              Cancel
            </Button>
          </div>
        ) : (
          <Button
            size="xs"
            variant="ghost"
            onClick={handleDelete}
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash2 className="size-3.5" />
          </Button>
        )}
      </div>

      {/* Sections */}
      {expanded && (
        <div className="border-t border-border">
          {sections.map((section) => (
            <SectionRow
              key={section.id}
              section={section}
              document={document}
              generation={sectionGeneration[section.id]}
              onRegenerate={() => actions.generateSection(document.id, section.id)}
              onUpdate={(content) => actions.updateSection(section.id, content, document.id)}
              onRestoreVersion={(versionId) =>
                actions.restoreVersion(section.id, versionId, document.id)
              }
              listVersions={() => actions.listVersions(section.id)}
            />
          ))}

          {/* Footer actions */}
          <div className="flex gap-2 border-t border-border px-4 py-2">
            <Button
              size="xs"
              variant="ghost"
              onClick={handleExport}
              className="text-muted-foreground"
            >
              <Download className="mr-1 size-3" />
              Export .md
            </Button>
            <Button
              size="xs"
              variant="ghost"
              onClick={handleCopyAll}
              className="text-muted-foreground"
            >
              <Copy className="mr-1 size-3" />
              Copy All
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

interface SectionRowProps {
  section: DocumentSection;
  document: Document;
  generation: SectionGenerationState | undefined;
  onRegenerate: () => void;
  onUpdate: (content: string) => void;
  onRestoreVersion: (versionId: string) => void;
  listVersions: () => Promise<import("./docs.types").SectionVersion[]>;
}

function SectionRow({
  section,
  generation,
  onRegenerate,
  onUpdate,
  onRestoreVersion,
  listVersions,
}: SectionRowProps) {
  const [sectionExpanded, setSectionExpanded] = useState(true);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(section.content);
  const [historyOpen, setHistoryOpen] = useState(false);

  const isGenerating = generation?.status === "generating";
  const hasError = generation?.status === "error";
  const displayContent =
    generation?.status === "generating" ? generation.accumulated : section.content;

  function handleEditSave() {
    onUpdate(editValue);
    setEditing(false);
  }

  function handleEditCancel() {
    setEditValue(section.content);
    setEditing(false);
  }

  return (
    <div className="border-b border-border/50 last:border-0">
      {/* Section header */}
      <div className="flex items-center gap-2 px-4 py-2">
        <button
          onClick={() => setSectionExpanded(!sectionExpanded)}
          className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
        >
          {sectionExpanded ? (
            <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate text-xs font-medium">{section.name}</span>
          {isGenerating && (
            <span className="text-xs text-muted-foreground animate-pulse">Generating...</span>
          )}
        </button>

        <div className="flex gap-0.5">
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setHistoryOpen(true)}
            className="size-6 p-0 text-muted-foreground"
            title="Version history"
          >
            <History className="size-3" />
          </Button>
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
              className="size-6 p-0 text-muted-foreground"
              title="Regenerate"
            >
              <RefreshCw className="size-3" />
            </Button>
          )}
          <Button
            size="xs"
            variant="ghost"
            onClick={() => {
              setEditValue(section.content);
              setEditing(true);
              setSectionExpanded(true);
            }}
            disabled={isGenerating || editing}
            className="size-6 p-0 text-muted-foreground"
            title="Edit"
          >
            <Pencil className="size-3" />
          </Button>
        </div>
      </div>

      {/* Section content */}
      {sectionExpanded && (
        <div className="px-4 pb-3">
          {hasError && generation?.status === "error" && (
            <div className="mb-2 flex items-center gap-2 rounded border border-destructive/30 bg-destructive/10 px-3 py-1.5 text-xs text-destructive-foreground">
              <span className="flex-1">{generation.message}</span>
              <Button size="xs" variant="ghost" onClick={onRegenerate} className="h-5 px-2">
                Retry
              </Button>
            </div>
          )}

          {editing ? (
            <div className="space-y-2">
              <textarea
                autoFocus
                value={editValue}
                onChange={(e) => setEditValue(e.target.value)}
                rows={8}
                className="w-full rounded border border-input bg-background px-2 py-1.5 font-mono text-xs focus:outline-none focus:ring-1 focus:ring-ring resize-y"
              />
              <div className="flex justify-end gap-1.5">
                <Button size="xs" variant="ghost" onClick={handleEditCancel}>
                  Cancel
                </Button>
                <Button size="xs" onClick={handleEditSave}>
                  Save
                </Button>
              </div>
            </div>
          ) : displayContent ? (
            <div className="prose prose-invert prose-sm max-w-none">
              <Markdown
                remarkPlugins={[remarkGfm]}
                components={{
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
              >{displayContent}</Markdown>
            </div>
          ) : !isGenerating ? (
            <p className="text-xs text-muted-foreground/60 italic">
              Empty — click regenerate to generate this section
            </p>
          ) : null}
        </div>
      )}

      {historyOpen && (
        <VersionHistory
          sectionName={section.name}
          listVersions={listVersions}
          onRestore={(versionId) => {
            onRestoreVersion(versionId);
            setHistoryOpen(false);
          }}
          onClose={() => setHistoryOpen(false)}
        />
      )}
    </div>
  );
}
