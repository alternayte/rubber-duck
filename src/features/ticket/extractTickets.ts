import type { CreateTicketParams } from "./ticket.types";

interface RawTicket {
  title?: string;
  description?: string;
  acceptance_criteria?: string;
  priority?: string;
  ticket_type?: string;
  estimate?: string;
  labels?: string[];
}

export function parseTicketsFromResponse(
  response: string,
  sessionId: string,
): { tickets: CreateTicketParams[]; error: string | null } {
  // 1. Try to find ```json blocks
  const jsonBlocks = extractJsonBlocks(response);

  if (jsonBlocks.length > 0) {
    const tickets: CreateTicketParams[] = [];
    for (const block of jsonBlocks) {
      try {
        const parsed = JSON.parse(block);
        const items = Array.isArray(parsed) ? parsed : [parsed];
        for (const item of items) {
          const ticket = normalizeTicket(item, sessionId);
          if (ticket) tickets.push(ticket);
        }
      } catch {
        // Skip unparseable blocks, try next one
        continue;
      }
    }
    if (tickets.length > 0) {
      return { tickets, error: null };
    }
  }

  // 2. Fallback: try parsing the entire response as JSON
  try {
    const parsed = JSON.parse(response);
    const items = Array.isArray(parsed) ? parsed : [parsed];
    const tickets: CreateTicketParams[] = [];
    for (const item of items) {
      const ticket = normalizeTicket(item, sessionId);
      if (ticket) tickets.push(ticket);
    }
    if (tickets.length > 0) {
      return { tickets, error: null };
    }
  } catch {
    // Not JSON
  }

  return { tickets: [], error: "Could not find ticket JSON in the response. Try asking again." };
}

function extractJsonBlocks(text: string): string[] {
  const blocks: string[] = [];
  const regex = /```(?:json)?\s*\n([\s\S]*?)```/g;
  let match;
  while ((match = regex.exec(text)) !== null) {
    blocks.push(match[1].trim());
  }
  return blocks;
}

function normalizeTicket(raw: RawTicket, sessionId: string): CreateTicketParams | null {
  if (!raw.title || typeof raw.title !== "string") return null;

  const validPriorities = ["Low", "Medium", "High", "Critical"];
  const validTypes = ["Task", "Bug", "Story", "Epic"];
  const validEstimates = ["XS", "S", "M", "L", "XL"];

  return {
    session_id: sessionId,
    title: raw.title.trim(),
    description: typeof raw.description === "string" ? raw.description : undefined,
    acceptance_criteria: typeof raw.acceptance_criteria === "string" ? raw.acceptance_criteria : undefined,
    priority: validPriorities.includes(raw.priority || "") ? raw.priority : undefined,
    ticket_type: validTypes.includes(raw.ticket_type || "") ? raw.ticket_type : undefined,
    estimate: validEstimates.includes(raw.estimate || "") ? raw.estimate : undefined,
    labels: Array.isArray(raw.labels) ? raw.labels.filter((l) => typeof l === "string") : undefined,
  };
}
