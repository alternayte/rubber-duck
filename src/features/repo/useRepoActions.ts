import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { reposAtom, indexStatusAtom } from "./repo.atoms";
import type { RepoContext } from "./repo.types";

export function useRepoActions() {
  const setRepos = useSetAtom(reposAtom);
  const setIndexStatus = useSetAtom(indexStatusAtom);

  async function loadRepos(sessionId: string) {
    const repos = await invoke<RepoContext[]>("list_repos", { sessionId });
    setRepos(repos);

    for (const repo of repos) {
      try {
        const status = await invoke<{ indexed: boolean; chunk_count: number }>(
          "get_index_status",
          { repoId: repo.id },
        );
        setIndexStatus((prev) => ({
          ...prev,
          [repo.id]: { indexed: status.indexed, chunkCount: status.chunk_count },
        }));
      } catch {
        // Ignore errors loading status
      }
    }
  }

  async function attachRepo(sessionId: string, source: string) {
    const repo = await invoke<RepoContext>("attach_repo", { sessionId, source });
    await loadRepos(sessionId);
    return repo;
  }

  async function detachRepo(repoId: string, sessionId: string) {
    await invoke<void>("detach_repo", { repoId });
    setIndexStatus((prev) => {
      const next = { ...prev };
      delete next[repoId];
      return next;
    });
    await loadRepos(sessionId);
  }

  async function reindexRepo(repoId: string) {
    setIndexStatus((prev) => ({
      ...prev,
      [repoId]: { indexing: true, progress: 0 },
    }));
    await invoke<void>("reindex_repo", { repoId });
  }

  return { loadRepos, attachRepo, detachRepo, reindexRepo };
}
