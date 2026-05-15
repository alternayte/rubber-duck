import { useEffect, useState } from "react";
import { useAtom, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Combobox } from "@/components/ui/combobox";
import type { ComboboxOption } from "@/components/ui/combobox";
import {
  apiKeySetAtom,
  selectedModelAtom,
  settingsOpenAtom,
  jiraConfiguredAtom,
  jiraDefaultProjectAtom,
} from "./settings.atoms";
import type { JiraProject } from "@/features/ticket/ticket.types";

interface ModelInfo {
  id: string;
  name: string;
  context_window: number;
}

export function SettingsDialog() {
  const [open, setOpen] = useAtom(settingsOpenAtom);
  const [apiKeySet, setApiKeySet] = useAtom(apiKeySetAtom);
  const [selectedModel, setSelectedModel] = useAtom(selectedModelAtom);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [saving, setSaving] = useState(false);

  const setJiraConfigured = useSetAtom(jiraConfiguredAtom);
  const [jiraDefaultProject, setJiraDefaultProject] = useAtom(jiraDefaultProjectAtom);

  const [jiraBaseUrl, setJiraBaseUrl] = useState("");
  const [jiraAuthMethod, setJiraAuthMethod] = useState("basic");
  const [jiraEmail, setJiraEmail] = useState("");
  const [jiraToken, setJiraToken] = useState("");
  const [showJiraToken, setShowJiraToken] = useState(false);
  const [jiraHasToken, setJiraHasToken] = useState(false);

  const [testingConnection, setTestingConnection] = useState(false);
  const [connectionResult, setConnectionResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [jiraProjects, setJiraProjects] = useState<ComboboxOption[]>([]);
  const [savingJira, setSavingJira] = useState(false);

  useEffect(() => {
    if (open) {
      invoke<boolean>("has_api_key").then(setApiKeySet);
      invoke<ModelInfo[]>("get_available_models").then(setModels);
      invoke<string | null>("get_setting", { key: "llm.model" }).then(
        (val) => {
          if (val) setSelectedModel(val);
        },
      );
      invoke<{ base_url: string; auth_method: string; email: string | null } | null>("get_jira_config").then((config) => {
        if (config) {
          setJiraBaseUrl(config.base_url);
          setJiraAuthMethod(config.auth_method);
          setJiraEmail(config.email ?? "");
        }
      });
      invoke<boolean>("has_jira_config").then((has) => {
        setJiraHasToken(has);
        setJiraConfigured(has);
      });
      invoke<string | null>("get_setting", { key: "jira.default_project" }).then((val) => {
        if (val) setJiraDefaultProject(val);
      });
      setJiraToken("");
      setShowJiraToken(false);
      setConnectionResult(null);
      setJiraProjects([]);
    }
  }, [open]);

  async function handleSaveApiKey() {
    if (!apiKeyInput.trim()) return;
    setSaving(true);
    await invoke("set_api_key", { key: apiKeyInput.trim() });
    setApiKeySet(true);
    setApiKeyInput("");
    setShowKey(false);
    setSaving(false);
  }

  async function handleModelChange(modelId: string) {
    setSelectedModel(modelId);
    await invoke("set_setting", {
      key: "llm.model",
      value: modelId,
      category: "llm",
    });
  }

  async function handleTestConnection() {
    setTestingConnection(true);
    setConnectionResult(null);

    // Save config first so backend can read it
    await invoke("set_jira_config", {
      baseUrl: jiraBaseUrl.trim(),
      authMethod: jiraAuthMethod,
      email: jiraAuthMethod === "basic" ? jiraEmail.trim() : null,
    });
    if (jiraToken) {
      await invoke("set_jira_api_token", { key: jiraToken });
    }

    try {
      const user = await invoke<{ display_name: string }>("test_jira_connection");
      setConnectionResult({ ok: true, message: `Connected as ${user.display_name}` });
      setJiraHasToken(true);
      setJiraConfigured(true);

      const projects = await invoke<JiraProject[]>("get_jira_projects");
      setJiraProjects(projects.map((p) => ({ value: p.key, label: `${p.key} - ${p.name}` })));
    } catch (err) {
      setConnectionResult({ ok: false, message: String(err) });
    } finally {
      setTestingConnection(false);
    }
  }

  async function handleSaveJira() {
    setSavingJira(true);
    await invoke("set_jira_config", {
      baseUrl: jiraBaseUrl.trim(),
      authMethod: jiraAuthMethod,
      email: jiraAuthMethod === "basic" ? jiraEmail.trim() : null,
    });
    if (jiraToken) {
      await invoke("set_jira_api_token", { key: jiraToken });
      setJiraHasToken(true);
    }
    if (jiraDefaultProject) {
      await invoke("set_setting", {
        key: "jira.default_project",
        value: jiraDefaultProject,
        category: "jira",
      });
    }
    const has = await invoke<boolean>("has_jira_config");
    setJiraConfigured(has);
    setSavingJira(false);
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-lg max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>

        <div className="space-y-6 py-4">
          <div className="space-y-2">
            <Label>OpenRouter API Key</Label>
            <div className="flex items-center gap-2">
              {apiKeySet && (
                <span className="text-xs text-green-500">✓ Key saved</span>
              )}
              {!apiKeySet && (
                <span className="text-xs text-destructive-foreground">
                  No key set
                </span>
              )}
            </div>
            <div className="flex gap-2">
              <Input
                type={showKey ? "text" : "password"}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder="sk-or-..."
                className="flex-1"
              />
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowKey(!showKey)}
              >
                {showKey ? "Hide" : "Show"}
              </Button>
            </div>
            <Button
              size="sm"
              onClick={handleSaveApiKey}
              disabled={!apiKeyInput.trim() || saving}
            >
              {saving ? "Saving..." : "Save Key"}
            </Button>
          </div>

          <div className="space-y-2">
            <Label>Model</Label>
            <Select value={selectedModel} onValueChange={handleModelChange}>
              <SelectTrigger>
                <SelectValue placeholder="Select a model" />
              </SelectTrigger>
              <SelectContent>
                {models.map((model) => (
                  <SelectItem key={model.id} value={model.id}>
                    {model.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Jira */}
          <div className="border-t border-border pt-4 space-y-3">
            <Label className="text-sm font-medium">Jira</Label>

            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Base URL</Label>
              <Input
                value={jiraBaseUrl}
                onChange={(e) => setJiraBaseUrl(e.target.value)}
                placeholder="https://jira.company.com"
                className="h-8 text-sm"
              />
            </div>

            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Auth Method</Label>
              <Select value={jiraAuthMethod} onValueChange={setJiraAuthMethod}>
                <SelectTrigger className="h-8 text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="basic">Basic (Cloud)</SelectItem>
                  <SelectItem value="pat">PAT (Server/DC)</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {jiraAuthMethod === "basic" && (
              <div className="space-y-1">
                <Label className="text-xs text-muted-foreground">Email</Label>
                <Input
                  value={jiraEmail}
                  onChange={(e) => setJiraEmail(e.target.value)}
                  placeholder="you@company.com"
                  className="h-8 text-sm"
                />
              </div>
            )}

            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">
                {jiraAuthMethod === "pat" ? "Personal Access Token" : "API Token"}
              </Label>
              <div className="flex gap-2">
                <Input
                  type={showJiraToken ? "text" : "password"}
                  value={jiraToken}
                  onChange={(e) => setJiraToken(e.target.value)}
                  placeholder={jiraHasToken ? "••••••••" : "Enter token"}
                  className="h-8 text-sm flex-1"
                />
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowJiraToken(!showJiraToken)}
                  className="h-8"
                >
                  {showJiraToken ? "Hide" : "Show"}
                </Button>
              </div>
            </div>

            <div className="space-y-1">
              <Button
                size="sm"
                variant="outline"
                onClick={handleTestConnection}
                disabled={testingConnection || !jiraBaseUrl.trim()}
                className="h-8"
              >
                {testingConnection ? "Testing..." : "Test Connection"}
              </Button>
              {connectionResult && (
                <p className={`text-xs ${connectionResult.ok ? "text-green-500" : "text-red-400"}`}>
                  {connectionResult.ok ? "✓" : "✗"} {connectionResult.message}
                </p>
              )}
            </div>

            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">Default Project</Label>
              <Combobox
                options={jiraProjects}
                value={jiraDefaultProject ?? ""}
                onValueChange={setJiraDefaultProject}
                placeholder="Test connection to load projects"
                searchPlaceholder="Search projects..."
                emptyText="No projects found."
                disabled={jiraProjects.length === 0}
              />
            </div>

            <Button
              size="sm"
              onClick={handleSaveJira}
              disabled={savingJira || !jiraBaseUrl.trim()}
              className="h-8"
            >
              {savingJira ? "Saving..." : "Save"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
