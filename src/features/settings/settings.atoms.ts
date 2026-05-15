import { atom } from "jotai";

export const apiKeySetAtom = atom(false);
export const selectedModelAtom = atom("deepseek/deepseek-chat-v4-0324:free");
export const settingsOpenAtom = atom(false);
export const jiraConfiguredAtom = atom(false);
export const jiraDefaultProjectAtom = atom<string | null>(null);
