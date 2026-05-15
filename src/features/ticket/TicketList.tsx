import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ticketsAtom } from "./ticket.atoms";
import { useTicketActions } from "./useTicketActions";
import type { Ticket } from "./ticket.types";
import { ChevronDown, ChevronRight, Plus, Trash2, ArrowUp, ArrowDown } from "lucide-react";

const PRIORITIES = ["Low", "Medium", "High", "Critical"];
const TYPES = ["Task", "Bug", "Story", "Epic"];
const ESTIMATES = ["XS", "S", "M", "L", "XL"];

interface TicketListProps {
  sessionId: string;
}

export function TicketList({ sessionId }: TicketListProps) {
  const tickets = useAtomValue(ticketsAtom);
  const { loadTickets, createTicket, updateTicket, deleteTicket, reorderTicket } = useTicketActions();
  const [collapsed, setCollapsed] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [editingTitleId, setEditingTitleId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const [isCreating, setIsCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");

  useEffect(() => {
    loadTickets(sessionId);
  }, [sessionId]);

  async function handleCreate() {
    const title = newTitle.trim();
    if (!title) return;
    await createTicket({ session_id: sessionId, title });
    setNewTitle("");
    setIsCreating(false);
  }

  async function handleTitleSave(ticket: Ticket) {
    const title = editingTitle.trim();
    if (title && title !== ticket.title) {
      await updateTicket(ticket.id, sessionId, { title });
    }
    setEditingTitleId(null);
  }

  function cycleBadge(ticket: Ticket, field: "priority" | "ticket_type" | "estimate", values: string[]) {
    const current = field === "estimate" ? ticket.estimate : field === "priority" ? ticket.priority : ticket.ticket_type;
    const currentIdx = values.indexOf(current || "");
    const nextIdx = (currentIdx + 1) % values.length;
    const nextValue = values[nextIdx];
    if (field === "estimate") {
      updateTicket(ticket.id, sessionId, { estimate: nextValue });
    } else if (field === "priority") {
      updateTicket(ticket.id, sessionId, { priority: nextValue });
    } else {
      updateTicket(ticket.id, sessionId, { ticket_type: nextValue });
    }
  }

  function handleDelete(ticket: Ticket) {
    if (window.confirm(`Delete "${ticket.title}"?`)) {
      deleteTicket(ticket.id, sessionId);
    }
  }

  function handleMoveUp(ticket: Ticket, index: number) {
    if (index === 0) return;
    reorderTicket(ticket.id, sessionId, tickets[index - 1].sort_order - 1);
  }

  function handleMoveDown(ticket: Ticket, index: number) {
    if (index >= tickets.length - 1) return;
    reorderTicket(ticket.id, sessionId, tickets[index + 1].sort_order + 1);
  }

  function priorityColor(priority: string) {
    switch (priority) {
      case "Critical": return "text-red-400 border-red-400/30";
      case "High": return "text-orange-400 border-orange-400/30";
      case "Medium": return "text-yellow-400 border-yellow-400/30";
      case "Low": return "text-blue-400 border-blue-400/30";
      default: return "text-muted-foreground border-border";
    }
  }

  function typeColor(type: string) {
    switch (type) {
      case "Bug": return "text-red-400 border-red-400/30";
      case "Story": return "text-green-400 border-green-400/30";
      case "Epic": return "text-purple-400 border-purple-400/30";
      default: return "text-muted-foreground border-border";
    }
  }

  return (
    <div className="border-t border-border pt-3 mt-3">
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-center gap-1 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors"
        >
          {collapsed ? <ChevronRight className="size-4" /> : <ChevronDown className="size-4" />}
          Tickets ({tickets.length})
        </button>
        <Button
          variant="ghost"
          size="xs"
          className="ml-auto text-muted-foreground"
          onClick={() => { setIsCreating(true); setCollapsed(false); }}
        >
          <Plus className="size-3" />
          Add
        </Button>
      </div>

      {!collapsed && (
        <div className="space-y-1">
          {/* Create inline */}
          {isCreating && (
            <div className="flex gap-2 mb-2">
              <Input
                autoFocus
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleCreate();
                  if (e.key === "Escape") { setIsCreating(false); setNewTitle(""); }
                }}
                placeholder="Ticket title..."
                className="h-7 text-sm flex-1"
              />
            </div>
          )}

          {tickets.length === 0 && !isCreating && (
            <p className="text-xs text-muted-foreground/60 py-2">
              No tickets yet — add one or ask the duck to extract them
            </p>
          )}

          {tickets.map((ticket, index) => (
            <div key={ticket.id} className="group rounded-md border border-border bg-card/50 px-3 py-2">
              {/* Top row: title + badges */}
              <div className="flex items-center gap-2">
                {editingTitleId === ticket.id ? (
                  <Input
                    autoFocus
                    value={editingTitle}
                    onChange={(e) => setEditingTitle(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleTitleSave(ticket);
                      if (e.key === "Escape") setEditingTitleId(null);
                    }}
                    onBlur={() => handleTitleSave(ticket)}
                    className="h-6 text-sm flex-1"
                  />
                ) : (
                  <span
                    className="flex-1 text-sm truncate cursor-pointer hover:text-foreground"
                    onClick={() => { setEditingTitleId(ticket.id); setEditingTitle(ticket.title); }}
                  >
                    {ticket.title}
                  </span>
                )}

                {/* Badges */}
                <button
                  onClick={() => cycleBadge(ticket, "ticket_type", TYPES)}
                  className={`rounded border px-1.5 py-0 text-[10px] ${typeColor(ticket.ticket_type)}`}
                >
                  {ticket.ticket_type}
                </button>
                <button
                  onClick={() => cycleBadge(ticket, "priority", PRIORITIES)}
                  className={`rounded border px-1.5 py-0 text-[10px] ${priorityColor(ticket.priority)}`}
                >
                  {ticket.priority}
                </button>
                <button
                  onClick={() => cycleBadge(ticket, "estimate", ESTIMATES)}
                  className="rounded border border-border px-1.5 py-0 text-[10px] text-muted-foreground"
                >
                  {ticket.estimate || "—"}
                </button>

                {/* Actions (visible on hover) */}
                <div className="hidden group-hover:flex items-center gap-0.5">
                  <button onClick={() => handleMoveUp(ticket, index)} className="p-0.5 text-muted-foreground hover:text-foreground" disabled={index === 0}>
                    <ArrowUp className="size-3" />
                  </button>
                  <button onClick={() => handleMoveDown(ticket, index)} className="p-0.5 text-muted-foreground hover:text-foreground" disabled={index >= tickets.length - 1}>
                    <ArrowDown className="size-3" />
                  </button>
                  <button onClick={() => handleDelete(ticket)} className="p-0.5 text-muted-foreground hover:text-red-400">
                    <Trash2 className="size-3" />
                  </button>
                </div>
              </div>

              {/* Expandable description */}
              {expandedId === ticket.id && (
                <div className="mt-2 space-y-2">
                  <div>
                    <label className="text-[10px] text-muted-foreground uppercase tracking-wider">Description</label>
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={ticket.description}
                      onChange={(e) => updateTicket(ticket.id, sessionId, { description: e.target.value })}
                    />
                  </div>
                  <div>
                    <label className="text-[10px] text-muted-foreground uppercase tracking-wider">Acceptance Criteria</label>
                    <textarea
                      className="mt-1 w-full rounded-md border border-input bg-background px-2 py-1 text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
                      rows={3}
                      value={ticket.acceptance_criteria}
                      onChange={(e) => updateTicket(ticket.id, sessionId, { acceptance_criteria: e.target.value })}
                    />
                  </div>
                </div>
              )}

              {/* Click to expand/collapse (only if not editing title) */}
              {editingTitleId !== ticket.id && (
                <button
                  onClick={() => setExpandedId(expandedId === ticket.id ? null : ticket.id)}
                  className="mt-1 text-[10px] text-muted-foreground/60 hover:text-muted-foreground"
                >
                  {expandedId === ticket.id ? "collapse" : "details..."}
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
