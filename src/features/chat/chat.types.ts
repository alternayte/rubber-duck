export interface RagChunkInfo {
  file_path: string;
  repo_name: string;
  start_line: number;
  end_line: number;
}

export interface ConversationMessage {
  id: string;
  role: "User" | "Assistant" | "System";
  content: string;
  created_at: string;
  rag_context: string | null;
}

export interface ChatThread {
  id: string;
  session_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}
