export interface ConversationMessage {
  id: string;
  role: "User" | "Assistant" | "System";
  content: string;
  created_at: string;
}
