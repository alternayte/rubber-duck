import { atom } from "jotai";
import type { RepoContext } from "./repo.types";

export const reposAtom = atom<RepoContext[]>([]);

export interface IndexStatus {
  indexed: boolean;
  chunkCount: number;
}

export interface IndexProgress {
  repo_id: string;
  repo_name: string;
  files_done: number;
  files_total: number;
}

export const indexStatusAtom = atom<
  Record<string, IndexStatus | { indexing: true; progress: number }>
>({});
