import { File } from "lucide-react";

const MENTION_SPLIT = /(@[\w.\-]+\/[\w.\-/]+)/g;
const MENTION_TEST = /^@[\w.\-]+\/[\w.\-/]+$/;

interface MentionTextProps {
  children: string;
}

export function MentionText({ children }: MentionTextProps) {
  const parts = children.split(MENTION_SPLIT);
  if (parts.length === 1) {
    return <>{children}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        MENTION_TEST.test(part) ? (
          <span
            key={i}
            className="inline-flex items-center gap-0.5 rounded bg-accent/50 px-1 py-0.5 text-xs font-mono text-accent-foreground"
          >
            <File className="size-3" />
            {part.slice(1)}
          </span>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}
