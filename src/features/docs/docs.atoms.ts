import { atom } from "jotai";
import type { Document, DocumentSection, SectionGenerationState } from "./docs.types";

export const documentsAtom = atom<Document[]>([]);
export const sectionsByDocAtom = atom<Record<string, DocumentSection[]>>({});
export const sectionGenerationAtom = atom<Record<string, SectionGenerationState>>({});
export const templatePickerOpenAtom = atom(false);
export const templateManagerOpenAtom = atom(false);
