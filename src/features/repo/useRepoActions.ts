import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { reposAtom } from "./repo.atoms";
import type { RepoContext } from "./repo.types";

export function useRepoActions() {
  const setRepos = useSetAtom(reposAtom);

  async function loadRepos(sessionId: string) {
    const repos = await invoke<RepoContext[]>("list_repos", { sessionId });
    setRepos(repos);
  }

  async function attachRepo(sessionId: string, source: string) {
    const repo = await invoke<RepoContext>("attach_repo", { sessionId, source });
    await loadRepos(sessionId);
    return repo;
  }

  async function detachRepo(repoId: string, sessionId: string) {
    await invoke<void>("detach_repo", { repoId });
    await loadRepos(sessionId);
  }

  return { loadRepos, attachRepo, detachRepo };
}
