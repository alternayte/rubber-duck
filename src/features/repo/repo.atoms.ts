import { atom } from "jotai";
import type { RepoContext } from "./repo.types";

export const reposAtom = atom<RepoContext[]>([]);
