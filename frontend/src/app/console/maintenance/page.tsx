"use client";

// Maintenance work-order board: every ticket with priority, status, and
// assignment. Gated by `maintenance:read`; status changes need `maintenance:manage`.

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { MaintenancePlan, MaintenanceTicket, Property } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";
import { toast } from "sonner";
import { Badge, Button, Card } from "@/components/ui";

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
      .catch((e) => logError("failed to load properties", e));
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
                  <Link
                    href={`/console/maintenance/${t.id}`}
                    className="block truncate font-semibold hover:underline"
                  >
                    {t.title}
                  </Link>
                  <div className="truncate text-sm text-ink-3">
                    {humanize(t.category)}
                    {t.reporter ? ` · ${t.reporter}` : ""}
                    {(t.sla_response_state === "breached" ||
                      t.sla_resolve_state === "breached") && (
                      <span className="ml-2 font-semibold text-bad">
                        SLA breached
                      </span>
                    )}
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

      <PlansCard properties={properties} manage={manage} />
    </div>
  );
}

/** Preventive-maintenance plans (Phase 6): recurring tasks the helpdesk scan
 *  turns into tickets on schedule. */
function PlansCard({
  properties,
  manage,
}: {
  properties: Property[];
  manage: boolean;
}) {
  const [plans, setPlans] = useState<MaintenancePlan[]>([]);
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [propertyId, setPropertyId] = useState("");
  const [title, setTitle] = useState("");
  const [cadence, setCadence] = useState("180");
  const [nextDue, setNextDue] = useState("");

  const load = () => {
    api
      .maintenancePlans()
      .then(setPlans)
      .catch((e) => logError("failed to load maintenance plans", e));
  };
  useEffect(load, []);

  const propName = useMemo(() => {
    const m = new Map(properties.map((p) => [p.id, p.name]));
    return (id: string) => m.get(id) ?? "—";
  }, [properties]);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const days = parseInt(cadence, 10);
    if (
      !propertyId ||
      !title.trim() ||
      !nextDue ||
      !Number.isFinite(days) ||
      days < 1
    ) {
      toast.error(
        "A plan needs a property, title, cadence, and first due date."
      );
      return;
    }
    await run(
      () =>
        api.createMaintenancePlan({
          property_id: propertyId,
          title: title.trim(),
          cadence_days: days,
          next_due_date: nextDue,
        }),
      "Plan created — the helpdesk scan opens its tickets on schedule."
    );
    setAdding(false);
    setTitle("");
    setNextDue("");
  }

  return (
    <Card>
      <div className="flex items-center justify-between border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">
          Preventive maintenance
        </h2>
        {manage && !adding && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setAdding(true)}
          >
            New plan
          </Button>
        )}
      </div>
      <div className="space-y-3 p-5 text-sm">
        {adding && (
          <form onSubmit={submit} className="flex flex-wrap items-center gap-2">
            <select
              className="rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink"
              value={propertyId}
              onChange={(e) => setPropertyId(e.target.value)}
            >
              <option value="">Property…</option>
              {properties.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <input
              className="w-64 rounded-xl border border-line bg-surface px-3 py-2 text-sm"
              placeholder="Task, e.g. “HVAC service”"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <label className="flex items-center gap-1 text-xs text-ink-3">
              every
              <input
                className="w-20 rounded-xl border border-line bg-surface px-3 py-2 text-sm"
                inputMode="numeric"
                value={cadence}
                onChange={(e) => setCadence(e.target.value)}
              />
              days
            </label>
            <label className="flex items-center gap-1 text-xs text-ink-3">
              first due
              <input
                type="date"
                className="rounded-xl border border-line bg-surface px-3 py-2 text-sm"
                value={nextDue}
                onChange={(e) => setNextDue(e.target.value)}
              />
            </label>
            <Button type="submit" disabled={busy}>
              Save
            </Button>
            <Button
              variant="ghost"
              type="button"
              onClick={() => setAdding(false)}
            >
              Cancel
            </Button>
          </form>
        )}
        {plans.length === 0 && !adding && (
          <p className="text-ink-3">
            No plans yet — schedule recurring work (HVAC service, gutters,
            detector checks) and tickets open themselves.
          </p>
        )}
        {plans.map((p) => (
          <div
            key={p.id}
            className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line px-4 py-3"
          >
            <div className="min-w-0">
              <div className="font-semibold">{p.title}</div>
              <div className="truncate text-xs text-ink-3">
                {propName(p.property_id)} · every {p.cadence_days} days · next{" "}
                {p.next_due_date}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Badge tone={p.active ? "good" : "neutral"}>
                {p.active ? "active" : "paused"}
              </Badge>
              {manage && (
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void run(
                      () =>
                        api.updateMaintenancePlan(p.id, { active: !p.active }),
                      p.active ? "Plan paused." : "Plan resumed."
                    )
                  }
                >
                  {p.active ? "Pause" : "Resume"}
                </Button>
              )}
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
