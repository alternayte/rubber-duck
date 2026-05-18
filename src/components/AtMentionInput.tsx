import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import getCaretCoordinates from "textarea-caret";
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
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number } | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
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

  function autoResize(el: HTMLTextAreaElement) {
    el.style.height = "auto";
    const maxHeight = window.innerHeight * 0.4;
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`;
  }

  function handleChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
    const newValue = e.target.value;
    onChange(newValue);

    autoResize(e.target);

    const cursorPos = e.target.selectionStart ?? newValue.length;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf("@");

    if (
      atIndex >= 0 &&
      (atIndex === 0 ||
        textBeforeCursor[atIndex - 1] === " " ||
        textBeforeCursor[atIndex - 1] === "\n")
    ) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(" ") && !query.includes("\n")) {
        setMentionStart(atIndex);
        setMentionQuery(query);
        setShowDropdown(true);
        search(query);
        const el = textareaRef.current;
        if (el) {
          const coords = getCaretCoordinates(el, atIndex);
          const rect = el.getBoundingClientRect();
          setDropdownPos({
            top: rect.top + coords.top - el.scrollTop,
            left: rect.left + coords.left,
          });
        }
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
    textareaRef.current?.focus();
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

    if (e.key === "Enter" && !e.shiftKey && !showDropdown) {
      e.preventDefault();
      onSubmit();
    }
  }

  // Auto-resize when value changes externally (e.g. cleared after submit)
  useEffect(() => {
    if (textareaRef.current) {
      autoResize(textareaRef.current);
    }
  }, [value]);

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  // Split display into directory path + filename for richer dropdown rendering
  function splitDisplay(display: string): { dir: string; file: string } {
    const lastSlash = display.lastIndexOf("/");
    if (lastSlash === -1) return { dir: "", file: display };
    return {
      dir: display.slice(0, lastSlash + 1),
      file: display.slice(lastSlash + 1),
    };
  }

  return (
    <div className="relative flex-1">
      <textarea
        ref={textareaRef}
        rows={1}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        className="w-full resize-none rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
      />
      {showDropdown && results.length > 0 && dropdownPos &&
        createPortal(
          <div
            className="fixed max-h-48 w-72 overflow-y-auto rounded-md border border-border bg-popover shadow-md z-[100]"
            style={{
              top: dropdownPos.top - 4,
              left: dropdownPos.left,
              transform: "translateY(-100%)",
            }}
          >
            {results.map((result, i) => {
              const { dir, file } = splitDisplay(result.display);
              return (
                <button
                  key={result.display}
                  onClick={() => selectResult(result)}
                  className={`w-full text-left px-3 py-1.5 text-xs truncate ${
                    i === selectedIndex
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50"
                  }`}
                >
                  <span className="text-muted-foreground">{dir}</span>
                  <span className={i === selectedIndex ? "text-accent-foreground" : "text-foreground"}>
                    {file}
                  </span>
                </button>
              );
            })}
          </div>,
          document.body,
        )}
    </div>
  );
}
