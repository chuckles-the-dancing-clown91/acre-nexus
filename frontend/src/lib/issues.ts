// The issue catalog: pick a problem, generate the ticket and shopping list.

import { request } from "@/lib/api";
import type { MaintenanceTicket } from "@/lib/types";
import type { PartsList } from "@/lib/parts";

export interface IssuePart {
  name: string;
  quantity: number;
  inventory_item_id: string | null;
}

export interface Issue {
  id: string;
  name: string;
  area: string | null;
  category: string;
  priority: string;
  description: string | null;
  est_minutes: number | null;
  checklist: string[];
  parts: IssuePart[];
  active: boolean;
}

export interface IssueInput {
  name: string;
  area?: string;
  category: string;
  priority: string;
  description?: string;
  est_minutes?: number;
  checklist: string[];
  parts: IssuePart[];
}

export interface Generated {
  ticket: MaintenanceTicket;
  parts: PartsList;
}

export const CATEGORIES = [
  "plumbing",
  "electrical",
  "hvac",
  "appliance",
  "structural",
  "general",
] as const;

export const issues = {
  list: () => request<Issue[]>("/issue-templates", { auth: true }),
  create: (body: IssueInput) =>
    request<Issue>("/issue-templates", { method: "POST", auth: true, body }),
  update: (id: string, body: IssueInput) =>
    request<Issue>(`/issue-templates/${id}`, {
      method: "PUT",
      auth: true,
      body,
    }),
  retire: (id: string) =>
    request<{ ok: boolean }>(`/issue-templates/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  generate: (
    id: string,
    body: {
      property_id: string;
      unit_id?: string;
      asset_id?: string;
      note?: string;
      priority?: string;
    }
  ) =>
    request<Generated>(`/issue-templates/${id}/generate`, {
      method: "POST",
      auth: true,
      body,
    }),
};
