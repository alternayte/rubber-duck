import { useEffect, useState } from "react";
import { useAtom } from "jotai";
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
import {
  apiKeySetAtom,
  selectedModelAtom,
  settingsOpenAtom,
} from "./settings.atoms";

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

  useEffect(() => {
    if (open) {
      invoke<boolean>("has_api_key").then(setApiKeySet);
      invoke<ModelInfo[]>("get_available_models").then(setModels);
      invoke<string | null>("get_setting", { key: "llm.model" }).then(
        (val) => {
          if (val) setSelectedModel(val);
        },
      );
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

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="sm:max-w-md">
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
        </div>
      </DialogContent>
    </Dialog>
  );
}
