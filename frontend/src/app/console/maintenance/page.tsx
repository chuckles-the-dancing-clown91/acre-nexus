"use client";

// Maintenance work-order board: every ticket with priority, status, and
// assignment. Gated by `maintenance:read`; status changes need `maintenance:manage`.

import { useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api";
import type { MaintenanceTicket, Property } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Card } from "@/components/ui";

const STATUSES = [
  "open",
  "triage",
  "scheduled",
  "in_progress",
  "on_hold",
  "resolved",
  "closed",
];

/** Tone for a ticket priority. */
function priorityTone(p: string): "bad" | "warn" | "info" | "neutral" {
  if (p === "urgent") return "bad";
  if (p === "high") return "warn";
  if (p === "normal") return "info";
  return "neutral";
}

/** Tone for a ticket status. */
function statusToneFor(s: string): "good" | "warn" | "info" | "neutral" {
  if (s === "resolved" || s === "closed") return "good";
  if (s === "in_progress" || s === "scheduled") return "info";
  if (s === "on_hold") return "warn";
  return "neutral";
}

function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function MaintenancePage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [tickets, setTickets] = useState<MaintenanceTicket[]>([]);
  const [properties, setProperties] = useState<Property[]>([]);
  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  const reload = () => {
    api
      .tickets(status ? { status } : {})
      .then(setTickets)
      .catch((e) => setError(e.message));
  };

  useEffect(() => {
    if (!can("maintenance:read")) return;
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, can]);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);

  const propName = useMemo(() => {
    const m = new Map(properties.map((p) => [p.id, p.name]));
    return (id: string) => m.get(id) ?? "—";
  }, [properties]);

  const setTicketStatus = async (id: string, next: string) => {
    try {
      await api.updateTicket(id, { status: next });
      reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (!can("maintenance:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to maintenance. Ask an admin for the{" "}
          <span className="font-mono">maintenance:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Maintenance
          </h1>
          <p className="text-ink-3">Work orders across the portfolio.</p>
        </div>
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
          >
            <option value="">All</option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {humanize(s)}
              </option>
            ))}
          </select>
        </label>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.6fr_1fr_.6fr_.8fr_.9fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Ticket</span>
          <span>Property</span>
          <span>Priority</span>
          <span>Status</span>
          <span className="text-right">Action</span>
        </div>
        <div className="divide-y divide-line">
          {tickets.length === 0 ? (
            <div className="px-5 py-10 text-center text-ink-3">
              No tickets{status ? " in this status" : ""}.
            </div>
          ) : (
            tickets.map((t) => (
              <div
                key={t.id}
                className="grid grid-cols-[1.6fr_1fr_.6fr_.8fr_.9fr] items-center gap-4 px-5 py-3.5"
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold">{t.title}</div>
                  <div className="truncate text-sm text-ink-3">
                    {humanize(t.category)}
                    {t.reporter ? ` · ${t.reporter}` : ""}
                  </div>
                </div>
                <span className="truncate text-sm text-ink-2">
                  {propName(t.property_id)}
                </span>
                <span>
                  <Badge tone={priorityTone(t.priority)}>{t.priority}</Badge>
                </span>
                <span>
                  <Badge tone={statusToneFor(t.status)}>
                    {humanize(t.status)}
                  </Badge>
                </span>
                <span className="flex justify-end">
                  {manage ? (
                    <select
                      value={t.status}
                      onChange={(e) => setTicketStatus(t.id, e.target.value)}
                      className="rounded-lg border border-line bg-surface px-2 py-1 text-xs text-ink"
                    >
                      {STATUSES.map((s) => (
                        <option key={s} value={s}>
                          {humanize(s)}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <span className="text-ink-3">—</span>
                  )}
                </span>
              </div>
            ))
          )}
        </div>
      </Card>
    </div>
  );
}
