import { useEffect, useState } from "react";
import { Copy, Pencil, Plus, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useDocActions } from "./useDocActions";
import type { BuiltinTemplate, Template } from "./docs.types";

interface TemplateManagerProps {
  onClose: () => void;
  onTemplatesChanged: () => void;
  builtinTemplates: BuiltinTemplate[];
}

type EditorMode =
  | { mode: "new" }
  | { mode: "edit"; template: Template }
  | null;

export function TemplateManager({
  onClose,
  onTemplatesChanged,
  builtinTemplates,
}: TemplateManagerProps) {
  const [customTemplates, setCustomTemplates] = useState<Template[]>([]);
  const [editorState, setEditorState] = useState<EditorMode>(null);
  const [editorName, setEditorName] = useState("");
  const [editorContent, setEditorContent] = useState("");
  const [saving, setSaving] = useState(false);

  const actions = useDocActions(undefined);

  useEffect(() => {
    actions.listCustomTemplates().then(setCustomTemplates);
  }, []);

  async function handleSave() {
    if (!editorName.trim() || !editorContent.trim()) return;
    setSaving(true);
    try {
      if (editorState?.mode === "edit") {
        await actions.updateCustomTemplate(
          editorState.template.id,
          editorName.trim(),
          editorContent.trim()
        );
      } else {
        await actions.createCustomTemplate(editorName.trim(), editorContent.trim());
      }
      const updated = await actions.listCustomTemplates();
      setCustomTemplates(updated);
      onTemplatesChanged();
      setEditorState(null);
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(id: string) {
    await actions.deleteCustomTemplate(id);
    const updated = await actions.listCustomTemplates();
    setCustomTemplates(updated);
    onTemplatesChanged();
  }

  function handleCloneBuiltin(builtin: BuiltinTemplate) {
    setEditorName(`${builtin.name} (custom)`);
    setEditorContent(builtin.content);
    setEditorState({ mode: "new" });
  }

  function handleEditCustom(template: Template) {
    setEditorName(template.name);
    setEditorContent(template.content);
    setEditorState({ mode: "edit", template });
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="relative flex h-[80vh] w-[700px] flex-col rounded-lg border border-border bg-card shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-medium">
            {editorState ? (editorState.mode === "edit" ? "Edit Template" : "New Template") : "Manage Templates"}
          </h2>
          <Button size="xs" variant="ghost" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>

        {editorState ? (
          /* Template editor */
          <div className="flex flex-1 flex-col gap-3 overflow-hidden p-4">
            <input
              type="text"
              value={editorName}
              onChange={(e) => setEditorName(e.target.value)}
              placeholder="Template name"
              className="rounded border border-input bg-background px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <div className="flex-1 overflow-hidden">
              <textarea
                value={editorContent}
                onChange={(e) => setEditorContent(e.target.value)}
                className="h-full w-full resize-none rounded border border-input bg-background p-3 font-mono text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                placeholder={`# Template Title\n<!-- section: Section Name -->\n<!-- directive: Instructions for the LLM to generate this section. -->`}
              />
            </div>
            <div className="flex justify-between">
              <p className="text-xs text-muted-foreground/60">
                Use <code>{"<!-- section: Name -->"}</code> and <code>{"<!-- directive: ... -->"}</code> markers
              </p>
              <div className="flex gap-2">
                <Button size="sm" variant="ghost" onClick={() => setEditorState(null)}>
                  Cancel
                </Button>
                <Button
                  size="sm"
                  onClick={handleSave}
                  disabled={saving || !editorName.trim() || !editorContent.trim()}
                >
                  {saving ? "Saving..." : "Save Template"}
                </Button>
              </div>
            </div>
          </div>
        ) : (
          /* Template list */
          <div className="flex flex-1 flex-col overflow-y-auto p-4 gap-4">
            {/* Built-in templates */}
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                Built-in Templates
              </p>
              <div className="space-y-1">
                {builtinTemplates.map((t) => (
                  <div
                    key={t.name}
                    className="flex items-center justify-between rounded px-3 py-2 hover:bg-accent/30"
                  >
                    <span className="text-sm">{t.name}</span>
                    <Button
                      size="xs"
                      variant="ghost"
                      onClick={() => handleCloneBuiltin(t)}
                      className="text-muted-foreground"
                    >
                      <Copy className="mr-1 size-3" />
                      Clone
                    </Button>
                  </div>
                ))}
              </div>
            </div>

            {/* Custom templates */}
            <div>
              <div className="mb-2 flex items-center justify-between">
                <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  Custom Templates
                </p>
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => {
                    setEditorName("");
                    setEditorContent("");
                    setEditorState({ mode: "new" });
                  }}
                >
                  <Plus className="mr-1 size-3" />
                  New
                </Button>
              </div>
              {customTemplates.length === 0 ? (
                <p className="text-xs text-muted-foreground/60 px-3">
                  No custom templates yet. Clone a built-in or create from scratch.
                </p>
              ) : (
                <div className="space-y-1">
                  {customTemplates.map((t) => (
                    <div
                      key={t.id}
                      className="flex items-center justify-between rounded px-3 py-2 hover:bg-accent/30"
                    >
                      <span className="text-sm">{t.name}</span>
                      <div className="flex gap-1">
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => handleEditCustom(t)}
                          className="text-muted-foreground"
                        >
                          <Pencil className="size-3" />
                        </Button>
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => handleDelete(t.id)}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-3" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
