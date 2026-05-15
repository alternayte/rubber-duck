export interface Ticket {
  id: string;
  session_id: string;
  title: string;
  description: string;
  acceptance_criteria: string;
  estimate: string | null;
  priority: string;
  ticket_type: string;
  labels: string[];
  parent_id: string | null;
  dependencies: string[];
  status: string;
  external_ref: string | null;
  sort_order: number;
  created_at: string;
}

export interface CreateTicketParams {
  session_id: string;
  title: string;
  description?: string;
  acceptance_criteria?: string;
  estimate?: string;
  priority?: string;
  ticket_type?: string;
  labels?: string[];
}

export interface UpdateTicketParams {
  title?: string;
  description?: string;
  acceptance_criteria?: string;
  estimate?: string;
  priority?: string;
  ticket_type?: string;
  labels?: string[];
  status?: string;
  parent_id?: string;
}
