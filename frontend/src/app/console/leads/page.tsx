"use client";

// Leasing CRM: leads fed by the monitored leasing inbox — mail to that address
// creates or updates a lead automatically. Gated by `application:read`; status
// changes need `application:write`.

import { useState } from "react";
import { useAuth } from "@/lib/auth";
import { useLeads, useUpdateLead } from "@/lib/queries";
import { Badge, Card } from "@/components/ui";

const STATUSES = ["new", "contacted", "toured", "applied", "closed"];

/** Tone for a lead status. */
function leadTone(
  status: string
): "neutral" | "good" | "warn" | "info" | "accent" {
  switch (status) {
    case "new":
      return "info";
    case "contacted":
      return "accent";
    case "toured":
      return "warn";
    case "applied":
      return "good";
    default:
      // closed
      return "neutral";
  }
}

function humanize(key: string): string {
  return key.charAt(0).toUpperCase() + key.slice(1);
}

export default function LeadsPage() {
  const { can } = useAuth();
  const write = can("application:write");
  const [status, setStatus] = useState<string>("");
  const { data, error, isLoading } = useLeads(status || undefined);
  const update = useUpdateLead();

  if (!can("application:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to leads. Ask an admin for the{" "}
          <span className="font-mono">application:read</span> permission.
        </p>
      </Card>
    );
  }

  const leads = data?.leads;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Leads
          </h1>
          <p className="text-ink-3">
            Prospective renters, from first email to signed application.
          </p>
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

      {data?.inbox_address && (
        <Card className="px-5 py-4 text-sm text-ink-2">
          Mail sent to{" "}
          <code className="rounded bg-surface-2 px-1.5 py-0.5 font-mono">
            {data.inbox_address}
          </code>{" "}
          creates or updates a lead automatically.
        </Card>
      )}

      {error && <p className="text-bad">{error.message}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.4fr_.8fr_.7fr_1.6fr_.7fr_.7fr_.9fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Lead</span>
          <span>Phone</span>
          <span>Source</span>
          <span>Last message</span>
          <span>Status</span>
          <span>Updated</span>
          <span className="text-right">Action</span>
        </div>
        <div className="divide-y divide-line">
          {leads?.map((l) => (
            <div
              key={l.id}
              className="grid grid-cols-[1.4fr_.8fr_.7fr_1.6fr_.7fr_.7fr_.9fr] items-center gap-4 px-5 py-3.5"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">{l.name}</div>
                <div className="truncate text-sm text-ink-3">{l.email}</div>
              </div>
              <span className="truncate text-sm text-ink-2">
                {l.phone ?? "—"}
              </span>
              <span className="truncate text-sm text-ink-2">{l.source}</span>
              <span className="line-clamp-2 text-sm text-ink-3">
                {l.last_message ?? "—"}
              </span>
              <span>
                <Badge tone={leadTone(l.status)}>{l.status}</Badge>
              </span>
              <span className="text-sm text-ink-2">
                {new Date(l.updated_at).toLocaleDateString()}
              </span>
              <span className="flex justify-end">
                {write ? (
                  <select
                    value={l.status}
                    disabled={update.isPending}
                    onChange={(e) =>
                      update.mutate({
                        id: l.id,
                        body: { status: e.target.value },
                      })
                    }
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
          ))}
          {isLoading && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {leads && leads.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No leads{status ? " in this status" : ""} — they arrive
              automatically from the leasing inbox.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
