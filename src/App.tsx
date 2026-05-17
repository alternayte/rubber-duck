import { useCallback, useEffect, useRef, useState } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { SessionSidebar } from "@/features/session/SessionSidebar";
import { DumpView } from "@/features/session/DumpView";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { apiKeySetAtom, jiraBaseUrlAtom, selectedModelAtom } from "@/features/settings/settings.atoms";
import { ChatPanel } from "@/features/chat/ChatPanel";
import { RepoPanel } from "@/features/repo/RepoPanel";
import { DocsView } from "@/features/docs/DocsView";

type Tab = "dump" | "docs" | "board";

const TABS: { id: Tab; label: string }[] = [
  { id: "dump", label: "Dump" },
  { id: "docs", label: "Docs" },
  { id: "board", label: "Board" },
];

const MIN_PANEL_WIDTH = 320;
const DEFAULT_PANEL_WIDTH = 420;
const MAX_PANEL_WIDTH = 800;

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("dump");
  const [sidePanelOpen, setSidePanelOpen] = useState(true);
  const [panelWidth, setPanelWidth] = useState(DEFAULT_PANEL_WIDTH);
  const isResizing = useRef(false);
  const activeSession = useAtomValue(activeSessionAtom);
  const setApiKeySet = useSetAtom(apiKeySetAtom);
  const setSelectedModel = useSetAtom(selectedModelAtom);
  const setJiraBaseUrl = useSetAtom(jiraBaseUrlAtom);

  useEffect(() => {
    invoke<boolean>("has_api_key").then(setApiKeySet);
    invoke<string | null>("get_setting", { key: "llm.model" }).then((val) => {
      if (val) setSelectedModel(val);
    });
    invoke<{ base_url: string; auth_method: string; email: string | null } | null>("get_jira_config").then((config) => {
      if (config) setJiraBaseUrl(config.base_url);
    });
  }, []);

  const handleMouseDown = useCallback(() => {
    isResizing.current = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing.current) return;
      const newWidth = window.innerWidth - e.clientX;
      setPanelWidth(Math.min(MAX_PANEL_WIDTH, Math.max(MIN_PANEL_WIDTH, newWidth)));
    };

    const handleMouseUp = () => {
      isResizing.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
  }, []);

  return (
    <div className="flex h-screen bg-background text-foreground select-none">
      <SessionSidebar />

      {/* Main area */}
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Tab bar */}
        <nav className="flex items-center gap-1 border-b border-border bg-card px-4 py-2">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
                activeTab === tab.id
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              }`}
            >
              {tab.label}
            </button>
          ))}

          <Button
            variant="ghost"
            size="sm"
            onClick={() => setSidePanelOpen(!sidePanelOpen)}
            className="ml-auto text-muted-foreground"
          >
            {sidePanelOpen ? "Panel →" : "← Panel"}
          </Button>
        </nav>

        {/* Tab content */}
        <main className="flex min-h-0 flex-1 flex-col p-6">
          {!activeSession ? (
            <p className="text-sm text-muted-foreground">
              Select or create a session to get started
            </p>
          ) : (
            <div className="min-h-0 flex-1">
              {activeTab === "dump" && (
                <DumpView sessionId={activeSession.id} />
              )}
              {activeTab === "docs" && (
                <DocsView sessionId={activeSession.id} />
              )}
              {activeTab === "board" && (
                <p className="text-sm text-muted-foreground">
                  Kanban board will go here
                </p>
              )}
            </div>
          )}
        </main>
      </div>

      {/* Side panel (context + chat) */}
      {sidePanelOpen && (
        <aside className="relative flex flex-col border-l border-border bg-card" style={{ width: panelWidth }}>
          <div
            onMouseDown={handleMouseDown}
            className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-accent/50 active:bg-accent z-10"
          />
          <RepoPanel />

          <ChatPanel />
        </aside>
      )}
      <SettingsDialog />
    </div>
  );
}

export default App;
