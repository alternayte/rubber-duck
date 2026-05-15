import { atom } from "jotai";
import type { Ticket } from "./ticket.types";

export const ticketsAtom = atom<Ticket[]>([]);
