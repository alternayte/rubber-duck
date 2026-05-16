import { useAtomValue } from "jotai";
import { openUrl } from "@tauri-apps/plugin-opener";
import { jiraBaseUrlAtom } from "@/features/settings/settings.atoms";

const TICKET_SPLIT = /([A-Z][A-Z0-9]+-\d+)/g;
const TICKET_TEST = /^[A-Z][A-Z0-9]+-\d+$/;

interface JiraLinkedTextProps {
  children: string;
}

export function JiraLinkedText({ children }: JiraLinkedTextProps) {
  const jiraBaseUrl = useAtomValue(jiraBaseUrlAtom);

  if (!jiraBaseUrl) {
    return <>{children}</>;
  }

  const parts = children.split(TICKET_SPLIT);
  if (parts.length === 1) {
    return <>{children}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        TICKET_TEST.test(part) ? (
          <button
            key={i}
            onClick={() => openUrl(`${jiraBaseUrl}/browse/${part}`)}
            className="text-blue-400 hover:text-blue-300 hover:underline cursor-pointer"
          >
            {part}
          </button>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}
