import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import { FileText, Plus, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { documentsAtom } from "./docs.atoms";
import { DocumentCard } from "./DocumentCard";
import { TemplateManager } from "./TemplateManager";
import { useDocActions } from "./useDocActions";
import type { BuiltinTemplate, Template, TemplateSection } from "./docs.types";

// Helper: parse section markers from template content
function parseTemplateSections(content: string): TemplateSection[] {
  const sections: TemplateSection[] = [];
  const lines = content.split("\n");
  let currentSection: string | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    const sectionMatch = trimmed.match(/^<!--\s*section:\s*(.+?)\s*-->$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      continue;
    }
    const directiveMatch = trimmed.match(/^<!--\s*directive:\s*([\s\S]+?)\s*-->$/);
    if (directiveMatch && currentSection) {
      sections.push({ name: currentSection, directive: directiveMatch[1] });
      currentSection = null;
    }
  }
  return sections;
}

interface DocsViewProps {
  sessionId: string;
}

export function DocsView({ sessionId }: DocsViewProps) {
  const documents = useAtomValue(documentsAtom);
  const [templateManagerOpen, setTemplateManagerOpen] = useState(false);
  const [builtinTemplates, setBuiltinTemplates] = useState<BuiltinTemplate[]>([]);
  const [customTemplates, setCustomTemplates] = useState<Template[]>([]);
  const [isCreating, setIsCreating] = useState(false);

  const actions = useDocActions(sessionId);

  useEffect(() => {
    actions.loadDocuments();
    actions.listBuiltinTemplates().then(setBuiltinTemplates);
    actions.listCustomTemplates().then(setCustomTemplates);
  }, [sessionId]);

  async function handleSelectTemplate(templateContent: string, templateName: string) {
    setIsCreating(true);
    try {
      const sections = parseTemplateSections(templateContent);
      const title = `${templateName} — ${new Date().toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
      const result = await actions.createDocument(
        templateName,
        title,
        sections.map((s, i) => ({ ...s, sort_order: i }))
      );
      if (!result) return;

      // Kick off sequential generation: one section at a time, top to bottom
      for (const section of result.sections) {
        await actions.generateSection(result.doc.id, section.id);
        // Brief pause between sections to avoid overwhelming the UI
        await new Promise((r) => setTimeout(r, 200));
      }
    } finally {
      setIsCreating(false);
    }
  }

  function refreshCustomTemplates() {
    actions.listCustomTemplates().then(setCustomTemplates);
  }

  if (documents.length === 0 && !isCreating) {
    return (
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-medium text-muted-foreground">Documents</h2>
          <GenerateButton
            builtinTemplates={builtinTemplates}
            customTemplates={customTemplates}
            onSelect={handleSelectTemplate}
            onManageTemplates={() => setTemplateManagerOpen(true)}
            disabled={isCreating}
          />
        </div>
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
          <FileText className="size-8 text-muted-foreground/40" />
          <p className="text-sm text-muted-foreground">No documents yet</p>
          <p className="text-xs text-muted-foreground/60">
            Generate a PRD, SDD, Test Plan, or ADR from your session context
          </p>
        </div>
        {templateManagerOpen && (
          <TemplateManager
            onClose={() => setTemplateManagerOpen(false)}
            onTemplatesChanged={refreshCustomTemplates}
            builtinTemplates={builtinTemplates}
          />
        )}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">
          Documents ({documents.length})
        </h2>
        <GenerateButton
          builtinTemplates={builtinTemplates}
          customTemplates={customTemplates}
          onSelect={handleSelectTemplate}
          onManageTemplates={() => setTemplateManagerOpen(true)}
          disabled={isCreating}
        />
      </div>

      {documents.map((doc) => (
        <DocumentCard
          key={doc.id}
          document={doc}
          sessionId={sessionId}
          onDeleted={actions.loadDocuments}
        />
      ))}

      {templateManagerOpen && (
        <TemplateManager
          onClose={() => setTemplateManagerOpen(false)}
          onTemplatesChanged={refreshCustomTemplates}
          builtinTemplates={builtinTemplates}
        />
      )}
    </div>
  );
}

interface GenerateButtonProps {
  builtinTemplates: BuiltinTemplate[];
  customTemplates: Template[];
  onSelect: (content: string, name: string) => void;
  onManageTemplates: () => void;
  disabled: boolean;
}

function GenerateButton({
  builtinTemplates,
  customTemplates,
  onSelect,
  onManageTemplates,
  disabled,
}: GenerateButtonProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" disabled={disabled}>
          <Plus className="mr-1 size-3.5" />
          Generate Document
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-52">
        {builtinTemplates.map((t) => (
          <DropdownMenuItem key={t.name} onSelect={() => onSelect(t.content, t.name)}>
            {t.name}
          </DropdownMenuItem>
        ))}
        {customTemplates.length > 0 && (
          <>
            <DropdownMenuSeparator />
            {customTemplates.map((t) => (
              <DropdownMenuItem key={t.id} onSelect={() => onSelect(t.content, t.name)}>
                {t.name}
              </DropdownMenuItem>
            ))}
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onManageTemplates} className="text-muted-foreground">
          <Settings className="mr-2 size-3.5" />
          Manage Templates
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
