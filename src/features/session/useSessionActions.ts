import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { sessionsAtom, activeSessionIdAtom } from "./session.atoms";
import type { Session } from "./session.types";

export function useSessionActions() {
  const setSessions = useSetAtom(sessionsAtom);
  const setActiveId = useSetAtom(activeSessionIdAtom);

  async function loadSessions() {
    const sessions = await invoke<Session[]>("list_sessions");
    setSessions(sessions);
  }

  async function createSession(title: string) {
    const session = await invoke<Session>("create_session", { title });
    await loadSessions();
    setActiveId(session.id);
    return session;
  }

  async function updateSession(id: string, title: string, context: string) {
    const session = await invoke<Session>("update_session", {
      id,
      title,
      context,
    });
    await loadSessions();
    return session;
  }

  async function archiveSession(id: string) {
    await invoke<Session>("archive_session", { id });
    await loadSessions();
    setActiveId(null);
  }

  return { loadSessions, createSession, updateSession, archiveSession };
}
