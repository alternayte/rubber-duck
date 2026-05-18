import { atom } from "jotai";
import type { ChatThread, ConversationMessage } from "./chat.types";

export const chatModeAtom = atom<"assist" | "grill">("assist");
export const isStreamingAtom = atom(false);
export const streamingContentAtom = atom("");
export const conversationAtom = atom<ConversationMessage[]>([]);
export const isExtractingAtom = atom(false);
export const ragContextAtom = atom<{ fileCount: number; repoCount: number } | null>(null);

export const chatThreadsAtom = atom<ChatThread[]>([]);
export const activeThreadIdAtom = atom<string | null>(null);

export const activeThreadAtom = atom((get) => {
  const threads = get(chatThreadsAtom);
  const id = get(activeThreadIdAtom);
  return id ? (threads.find((t) => t.id === id) ?? null) : null;
});
