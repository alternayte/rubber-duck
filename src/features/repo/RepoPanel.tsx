import { useEffect, useState } from "react";
import { useAtom } from "jotai";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { reposAtom } from "./repo.atoms";
import { useRepoActions } from "./useRepoActions";
import { FileTree } from "./FileTree";
import { Plus, FolderOpen, GitBranch, X, Loader2 } from "lucide-react";

export function RepoPanel() {
  const [activeSession] = useAtom(activeSessionAtom);
  const [repos] = useAtom(reposAtom);
  const { loadRepos, attachRepo, detachRepo } = useRepoActions();
  const [showAttach, setShowAttach] = useState(false);
  const [gitUrl, setGitUrl] = useState("");
  const [attaching, setAttaching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (activeSession) {
      loadRepos(activeSession.id);
    }
  }, [activeSession?.id]);

  async function handleAttachLocal() {
    if (!activeSession) return;
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;

    setAttaching(true);
    setError(null);
    try {
      await attachRepo(activeSession.id, selected as string);
      setShowAttach(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setAttaching(false);
    }
  }

  async function handleAttachGit() {
    if (!activeSession || !gitUrl.trim()) return;

    setAttaching(true);
    setError(null);
    try {
      await attachRepo(activeSession.id, gitUrl.trim());
      setGitUrl("");
      setShowAttach(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setAttaching(false);
    }
  }

  if (!activeSession) {
    return (
      <div className="border-b border-border p-4">
        <h2 className="text-sm font-medium text-muted-foreground">Context</h2>
        <p className="mt-2 text-xs text-muted-foreground/60">Select a session first</p>
      </div>
    );
  }

  return (
    <div className="border-b border-border p-4 space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">Context</h2>
        <Button
          variant="ghost"
          size="xs"
          onClick={() => setShowAttach(!showAttach)}
          className="text-muted-foreground"
        >
          <Plus className="size-3" />
        </Button>
      </div>

      {showAttach && (
        <div className="space-y-2 rounded-md border border-border p-2">
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="xs"
              onClick={handleAttachLocal}
              disabled={attaching}
              className="flex-1"
            >
              <FolderOpen className="size-3 mr-1" />
              Local Folder
            </Button>
          </div>
          <div className="flex gap-2">
            <Input
              value={gitUrl}
              onChange={(e) => setGitUrl(e.target.value)}
              placeholder="https://github.com/..."
              className="h-7 text-xs flex-1"
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAttachGit();
              }}
            />
            <Button
              variant="outline"
              size="xs"
              onClick={handleAttachGit}
              disabled={attaching || !gitUrl.trim()}
            >
              {attaching ? <Loader2 className="size-3 animate-spin" /> : <GitBranch className="size-3" />}
            </Button>
          </div>
          {error && <p className="text-[10px] text-red-400">{error}</p>}
        </div>
      )}

      {repos.length === 0 && !showAttach && (
        <p className="text-xs text-muted-foreground/60">No repos attached</p>
      )}

      {repos.map((repo) => (
        <div key={repo.id} className="space-y-1">
          <div className="flex items-center gap-1 text-xs">
            {repo.source.startsWith("http") || repo.source.startsWith("git@") || repo.source.startsWith("ssh://") ? (
              <GitBranch className="size-3 text-muted-foreground" />
            ) : (
              <FolderOpen className="size-3 text-muted-foreground" />
            )}
            <span className="text-muted-foreground flex-1 truncate">{repo.name}</span>
            <button
              onClick={() => detachRepo(repo.id, activeSession.id)}
              className="text-muted-foreground/50 hover:text-red-400"
            >
              <X className="size-3" />
            </button>
          </div>
          <FileTree repoId={repo.id} repoName={repo.name} />
        </div>
      ))}
    </div>
  );
}
