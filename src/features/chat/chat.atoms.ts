import { atom } from "jotai";
import type { ConversationMessage } from "./chat.types";

export const chatModeAtom = atom<"assist" | "grill">("assist");
export const isStreamingAtom = atom(false);
export const streamingContentAtom = atom("");
export const conversationAtom = atom<ConversationMessage[]>([]);
