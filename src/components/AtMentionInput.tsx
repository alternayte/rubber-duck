import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Input } from "@/components/ui/input";
import type { FileSearchResult } from "@/features/repo/repo.types";

interface AtMentionInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  sessionId: string;
  placeholder?: string;
  disabled?: boolean;
}

export function AtMentionInput({
  value,
  onChange,
  onSubmit,
  sessionId,
  placeholder,
  disabled,
}: AtMentionInputProps) {
  const [showDropdown, setShowDropdown] = useState(false);
  const [results, setResults] = useState<FileSearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStart, setMentionStart] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const search = useCallback(
    (query: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        if (query.length < 1) {
          setResults([]);
          return;
        }
        const res = await invoke<FileSearchResult[]>("search_repo_files", {
          sessionId,
          query,
        });
        setResults(res);
        setSelectedIndex(0);
      }, 200);
    },
    [sessionId],
  );

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const newValue = e.target.value;
    onChange(newValue);

    const cursorPos = e.target.selectionStart ?? newValue.length;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf("@");

    if (atIndex >= 0 && (atIndex === 0 || textBeforeCursor[atIndex - 1] === " ")) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(" ")) {
        setMentionStart(atIndex);
        setMentionQuery(query);
        setShowDropdown(true);
        search(query);
        return;
      }
    }

    setShowDropdown(false);
  }

  function selectResult(result: FileSearchResult) {
    const before = value.slice(0, mentionStart);
    const after = value.slice(mentionStart + 1 + mentionQuery.length);
    onChange(`${before}@${result.display}${after} `);
    setShowDropdown(false);
    inputRef.current?.focus();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (showDropdown && results.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        selectResult(results[selectedIndex]);
        return;
      }
      if (e.key === "Escape") {
        setShowDropdown(false);
        return;
      }
    }

    if (e.key === "Enter" && !showDropdown) {
      e.preventDefault();
      onSubmit();
    }
  }

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  return (
    <div className="relative flex-1">
      <Input
        ref={inputRef}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        className="text-sm"
      />
      {showDropdown && results.length > 0 && (
        <div className="absolute bottom-full left-0 right-0 mb-1 max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md z-50">
          {results.map((result, i) => (
            <button
              key={result.display}
              onClick={() => selectResult(result)}
              className={`w-full text-left px-3 py-1.5 text-xs truncate ${
                i === selectedIndex
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50"
              }`}
            >
              {result.display}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
