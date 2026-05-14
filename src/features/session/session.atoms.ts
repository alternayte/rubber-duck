import { atom } from "jotai";
import type { Session } from "./session.types";

export const sessionsAtom = atom<Session[]>([]);
export const activeSessionIdAtom = atom<string | null>(null);

export const activeSessionAtom = atom((get) => {
  const sessions = get(sessionsAtom);
  const id = get(activeSessionIdAtom);
  return id ? (sessions.find((s) => s.id === id) ?? null) : null;
});
