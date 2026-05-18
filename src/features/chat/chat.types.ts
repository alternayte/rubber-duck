export interface ConversationMessage {
  id: string;
  role: "User" | "Assistant" | "System";
  content: string;
  created_at: string;
}

export interface ChatThread {
  id: string;
  session_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}
