export type SessionStatus = "Draft" | "Active" | "Archived";

export interface Session {
  id: string;
  title: string;
  context: string;
  status: SessionStatus;
  created_at: string;
  updated_at: string;
}
