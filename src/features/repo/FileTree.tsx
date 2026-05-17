import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, File, Folder } from "lucide-react";
import type { FileNode } from "./repo.types";

interface FileTreeProps {
  repoId: string;
  repoName: string;
}

export function FileTree({ repoId, repoName }: FileTreeProps) {
  const [tree, setTree] = useState<FileNode[] | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [loading, setLoading] = useState(false);

  async function handleToggle() {
    if (!expanded && tree === null) {
      setLoading(true);
      const nodes = await invoke<FileNode[]>("get_repo_tree", { repoId });
      setTree(nodes);
      setLoading(false);
    }
    setExpanded(!expanded);
  }

  return (
    <div className="text-xs">
      <button
        onClick={handleToggle}
        className="flex items-center gap-1 w-full text-left py-0.5 text-muted-foreground hover:text-foreground"
      >
        {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
        <Folder className="size-3" />
        <span>{repoName}/</span>
      </button>
      {loading && <p className="pl-6 text-muted-foreground/60 animate-pulse">Loading...</p>}
      {expanded && tree && (
        <div className="pl-3">
          {tree.map((node) => (
            <TreeNode key={node.path} node={node} />
          ))}
        </div>
      )}
    </div>
  );
}

function TreeNode({ node }: { node: FileNode }) {
  const [expanded, setExpanded] = useState(false);

  if (node.is_dir) {
    return (
      <div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 w-full text-left py-0.5 text-muted-foreground hover:text-foreground"
        >
          {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
          <Folder className="size-3 text-blue-400/70" />
          <span>{node.name}/</span>
        </button>
        {expanded && node.children.length > 0 && (
          <div className="pl-3">
            {node.children.map((child) => (
              <TreeNode key={child.path} node={child} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1 py-0.5 pl-4 text-muted-foreground/80">
      <File className="size-3" />
      <span>{node.name}</span>
    </div>
  );
}
