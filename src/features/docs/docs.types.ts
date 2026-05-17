export interface Document {
  id: string;
  session_id: string;
  template_name: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface DocumentSection {
  id: string;
  document_id: string;
  name: string;
  directive: string;
  content: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface SectionVersion {
  id: string;
  section_id: string;
  content: string;
  created_at: string;
}

export interface Template {
  id: string;
  name: string;
  content: string;
  created_at: string;
  updated_at: string;
}

export interface BuiltinTemplate {
  name: string;
  content: string;
}

export interface TemplateSection {
  name: string;
  directive: string;
}

export type SectionGenerationState =
  | { status: "idle" }
  | { status: "generating"; accumulated: string }
  | { status: "error"; message: string };
