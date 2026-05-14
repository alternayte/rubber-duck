import { useEffect, useState } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { SessionSidebar } from "@/features/session/SessionSidebar";
import { DumpView } from "@/features/session/DumpView";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { apiKeySetAtom, selectedModelAtom } from "@/features/settings/settings.atoms";

type Tab = "dump" | "refine" | "board";

const TABS: { id: Tab; label: string }[] = [
  { id: "dump", label: "Dump" },
  { id: "refine", label: "Refine" },
  { id: "board", label: "Board" },
];

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("dump");
  const [sidePanelOpen, setSidePanelOpen] = useState(true);
  const activeSession = useAtomValue(activeSessionAtom);
  const setApiKeySet = useSetAtom(apiKeySetAtom);
  const setSelectedModel = useSetAtom(selectedModelAtom);

  useEffect(() => {
    invoke<boolean>("has_api_key").then(setApiKeySet);
    invoke<string | null>("get_setting", { key: "llm.model" }).then((val) => {
      if (val) setSelectedModel(val);
    });
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
              {activeTab === "refine" && (
                <p className="text-sm text-muted-foreground">
                  Split pane refine view will go here
                </p>
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
        <aside className="flex w-80 flex-col border-l border-border bg-card">
          {/* Context section */}
          <div className="border-b border-border p-4">
            <h2 className="text-sm font-medium text-muted-foreground">
              Context
            </h2>
            <p className="mt-2 text-xs text-muted-foreground/60">
              No repos or files attached
            </p>
          </div>

          {/* Chat section */}
          <div className="flex min-h-0 flex-1 flex-col">
            <div className="flex items-center gap-2 border-b border-border px-4 py-2">
              <h2 className="text-sm font-medium text-muted-foreground">
                Duck Chat
              </h2>
              <div className="ml-auto flex gap-1">
                <Button variant="secondary" size="xs">
                  Assist
                </Button>
                <Button variant="ghost" size="xs" className="text-muted-foreground">
                  Grill
                </Button>
              </div>
            </div>
            <div className="flex-1 overflow-y-auto p-4">
              <p className="text-xs text-muted-foreground/60">
                Chat messages will appear here
              </p>
            </div>
            <div className="border-t border-border p-3">
              <input
                type="text"
                placeholder="Ask the duck..."
                disabled
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-muted-foreground placeholder-muted-foreground/60"
              />
            </div>
          </div>
        </aside>
      )}
      <SettingsDialog />
    </div>
  );
}

export default App;
