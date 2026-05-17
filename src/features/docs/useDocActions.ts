import { useSetAtom, useAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { documentsAtom, sectionsByDocAtom, sectionGenerationAtom } from "./docs.atoms";
import type {
  Document,
  DocumentSection,
  BuiltinTemplate,
  Template,
  SectionVersion,
} from "./docs.types";

export function useDocActions(sessionId: string | undefined) {
  const setDocuments = useSetAtom(documentsAtom);
  const setSectionsByDoc = useSetAtom(sectionsByDocAtom);
  const [sectionGeneration, setSectionGeneration] = useAtom(sectionGenerationAtom);

  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<{ section_id: string; content: string }>("doc:chunk", (event) => {
      const { section_id, content } = event.payload;
      setSectionGeneration((prev) => {
        const current = prev[section_id];
        const accumulated =
          current?.status === "generating" ? current.accumulated + content : content;
        return { ...prev, [section_id]: { status: "generating" as const, accumulated } };
      });
    }).then((u) => unlisteners.push(u));

    listen<{ section_id: string; full_content: string }>("doc:done", (event) => {
      const { section_id } = event.payload;
      setSectionGeneration((prev) => {
        const next = { ...prev };
        delete next[section_id];
        return next;
      });
      setSectionsByDoc((prev) => {
        for (const [docId, sections] of Object.entries(prev)) {
          if (sections.some((s) => s.id === section_id)) {
            invoke<DocumentSection[]>("list_document_sections", { documentId: docId }).then(
              (updated) => {
                setSectionsByDoc((p) => ({ ...p, [docId]: updated }));
              },
            );
            break;
          }
        }
        return prev;
      });
    }).then((u) => unlisteners.push(u));

    listen<{ section_id: string; message: string }>("doc:error", (event) => {
      const { section_id, message } = event.payload;
      setSectionGeneration((prev) => ({
        ...prev,
        [section_id]: { status: "error" as const, message },
      }));
    }).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((fn) => fn());
  }, []);

  async function loadDocuments() {
    if (!sessionId) return;
    const docs = await invoke<Document[]>("list_docs", { sessionId });
    setDocuments(docs);
  }

  async function loadSections(documentId: string) {
    const sections = await invoke<DocumentSection[]>("list_document_sections", { documentId });
    setSectionsByDoc((prev) => ({ ...prev, [documentId]: sections }));
  }

  async function createDocument(
    templateName: string,
    title: string,
    sections: Array<{ name: string; directive: string; sort_order: number }>,
  ) {
    if (!sessionId) return;
    const [doc, secs] = await invoke<[Document, DocumentSection[]]>("create_doc", {
      sessionId,
      templateName,
      title,
      sections,
    });
    await loadDocuments();
    setSectionsByDoc((prev) => ({ ...prev, [doc.id]: secs }));
    return { doc, sections: secs };
  }

  async function deleteDocument(documentId: string) {
    await invoke("delete_doc", { documentId });
    setSectionsByDoc((prev) => {
      const next = { ...prev };
      delete next[documentId];
      return next;
    });
    await loadDocuments();
  }

  async function generateSection(documentId: string, sectionId: string) {
    setSectionGeneration((prev) => ({
      ...prev,
      [sectionId]: { status: "generating" as const, accumulated: "" },
    }));
    try {
      await invoke("generate_doc_section", { documentId, sectionId });
    } catch (err) {
      setSectionGeneration((prev) => ({
        ...prev,
        [sectionId]: { status: "error" as const, message: String(err) },
      }));
    }
  }

  async function updateSection(sectionId: string, content: string, documentId: string) {
    await invoke<DocumentSection>("update_document_section", { sectionId, content });
    await loadSections(documentId);
  }

  async function listVersions(sectionId: string) {
    return invoke<SectionVersion[]>("list_section_versions", { sectionId });
  }

  async function restoreVersion(sectionId: string, versionId: string, documentId: string) {
    await invoke<DocumentSection>("restore_section_version", { sectionId, versionId });
    await loadSections(documentId);
  }

  async function listBuiltinTemplates() {
    return invoke<BuiltinTemplate[]>("list_builtin_templates");
  }

  async function listCustomTemplates() {
    return invoke<Template[]>("list_custom_templates");
  }

  async function createCustomTemplate(name: string, content: string) {
    return invoke<Template>("create_custom_template", { name, content });
  }

  async function updateCustomTemplate(templateId: string, name: string, content: string) {
    return invoke<Template>("update_custom_template", { templateId, name, content });
  }

  async function deleteCustomTemplate(templateId: string) {
    await invoke("delete_custom_template", { templateId });
  }

  return {
    loadDocuments,
    loadSections,
    createDocument,
    deleteDocument,
    generateSection,
    updateSection,
    listVersions,
    restoreVersion,
    listBuiltinTemplates,
    listCustomTemplates,
    createCustomTemplate,
    updateCustomTemplate,
    deleteCustomTemplate,
    sectionGeneration,
  };
}
