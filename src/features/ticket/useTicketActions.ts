import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { ticketsAtom } from "./ticket.atoms";
import type { Ticket, CreateTicketParams, UpdateTicketParams } from "./ticket.types";

export function useTicketActions() {
  const setTickets = useSetAtom(ticketsAtom);

  async function loadTickets(sessionId: string) {
    const tickets = await invoke<Ticket[]>("list_tickets", { sessionId });
    setTickets(tickets);
  }

  async function createTicket(params: CreateTicketParams) {
    const ticket = await invoke<Ticket>("create_ticket", { params });
    await loadTickets(params.session_id);
    return ticket;
  }

  async function updateTicket(id: string, sessionId: string, params: UpdateTicketParams) {
    const ticket = await invoke<Ticket>("update_ticket", { id, params });
    await loadTickets(sessionId);
    return ticket;
  }

  async function deleteTicket(id: string, sessionId: string) {
    await invoke<void>("delete_ticket", { id });
    await loadTickets(sessionId);
  }

  async function reorderTicket(id: string, sessionId: string, sortOrder: number) {
    await invoke<void>("reorder_ticket", { id, sortOrder });
    await loadTickets(sessionId);
  }

  async function pushToJira(ticketId: string, sessionId: string, projectKey: string) {
    const ticket = await invoke<Ticket>("push_ticket_to_jira", { ticketId, projectKey });
    await loadTickets(sessionId);
    return ticket;
  }

  return { loadTickets, createTicket, updateTicket, deleteTicket, reorderTicket, pushToJira };
}
