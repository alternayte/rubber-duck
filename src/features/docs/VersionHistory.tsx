import { useEffect, useState } from "react";
import { X } from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import type { SectionVersion } from "./docs.types";

interface VersionHistoryProps {
  sectionName: string;
  listVersions: () => Promise<SectionVersion[]>;
  onRestore: (versionId: string) => void;
  onClose: () => void;
}

export function VersionHistory({
  sectionName,
  listVersions,
  onRestore,
  onClose,
}: VersionHistoryProps) {
  const [versions, setVersions] = useState<SectionVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [previewVersion, setPreviewVersion] = useState<SectionVersion | null>(null);

  useEffect(() => {
    listVersions()
      .then((v) => {
        setVersions(v);
        if (v.length > 0) setPreviewVersion(v[0]);
      })
      .finally(() => setLoading(false));
  }, []);

  function formatDate(isoString: string): string {
    const date = new Date(isoString);
    return date.toLocaleString("en-US", {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="relative flex h-[70vh] w-[680px] flex-col rounded-lg border border-border bg-card shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-medium">
            Version History — {sectionName}
          </h2>
          <Button size="xs" variant="ghost" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>

        {loading ? (
          <div className="flex flex-1 items-center justify-center">
            <p className="text-sm text-muted-foreground animate-pulse">Loading...</p>
          </div>
        ) : versions.length === 0 ? (
          <div className="flex flex-1 items-center justify-center">
            <p className="text-sm text-muted-foreground">No saved versions yet</p>
          </div>
        ) : (
          <div className="flex min-h-0 flex-1">
            {/* Version list */}
            <div className="w-56 shrink-0 overflow-y-auto border-r border-border">
              {versions.map((v) => (
                <button
                  key={v.id}
                  onClick={() => setPreviewVersion(v)}
                  className={`w-full px-4 py-3 text-left text-xs border-b border-border/50 hover:bg-accent/30 transition-colors ${
                    previewVersion?.id === v.id ? "bg-accent/50" : ""
                  }`}
                >
                  {formatDate(v.created_at)}
                </button>
              ))}
            </div>

            {/* Preview */}
            <div className="flex flex-1 flex-col overflow-hidden">
              <div className="flex-1 overflow-y-auto p-4">
                {previewVersion && (
                  <div className="prose prose-invert prose-sm max-w-none">
                    <Markdown remarkPlugins={[remarkGfm]}>
                      {previewVersion.content}
                    </Markdown>
                  </div>
                )}
              </div>
              {previewVersion && (
                <div className="border-t border-border px-4 py-3 flex justify-end">
                  <Button size="sm" onClick={() => onRestore(previewVersion.id)}>
                    Restore This Version
                  </Button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
