"use client";

// Work-order detail (Phase 6): the full helpdesk view of one ticket — SLA
// targets and state, status/priority/scheduling controls, dispatch (member
// or contractor assignment with notifications), the comment timeline,
// contractor quotes → approval (feeding the vendor-bill prefill), a
// one-click bill from the resolved ticket, and attachments.

import { useCallback, useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import { api, iam, type Member } from "@/lib/api";
import type { Asset, Counterparty, TicketDetail } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { logError } from "@/lib/log";
import { toast } from "sonner";
import { Badge, Button, Card, statusTone } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";

const STATUSES = [
  "open",
  "triage",
  "scheduled",
  "in_progress",
  "on_hold",
  "resolved",
  "closed",
];
const PRIORITIES = ["low", "normal", "high", "urgent"];

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";
const select =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function slaTone(state: string): "good" | "warn" | "bad" | "neutral" {
  if (state === "met") return "good";
  if (state === "on_track") return "warn";
  if (state === "breached") return "bad";
  return "neutral";
}

function fmtWhen(iso: string | null) {
  return iso ? iso.slice(0, 16).replace("T", " ") : "—";
}

export default function TicketDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;
  const router = useRouter();
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const approve = can("payable:approve");

  const [ticket, setTicket] = useState<TicketDetail | null>(null);
  const [members, setMembers] = useState<Member[]>([]);
  const [contractors, setContractors] = useState<Counterparty[]>([]);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [comment, setComment] = useState("");
  const [noteMode, setNoteMode] = useState<"public" | "internal">("public");

  const load = useCallback(() => {
    api
      .ticket(id)
      .then((t) => {
        setTicket(t);
        api
          .assets({ property_id: t.property_id, status: "active" })
          .then(setAssets)
          .catch((e) => logError("failed to load assets", e));
      })
      .catch((e) => setError(e.message));
  }, [id]);

  useEffect(() => {
    load();
    iam
      .members()
      .then(setMembers)
      .catch((e) => logError("failed to load members", e));
    api
      .entities("contractor")
      .then(setContractors)
      .catch((e) => logError("failed to load contractors", e));
  }, [load]);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    setError(null);
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

  if (!ticket) {
    return (
      <div className="space-y-4">
        {error ? (
          <p className="text-bad">{error}</p>
        ) : (
          <p className="text-ink-3">Loading…</p>
        )}
      </div>
    );
  }

  async function sendComment(e: React.FormEvent) {
    e.preventDefault();
    if (!comment.trim() || !ticket) return;
    await run(async () => {
      await api.addTicketComment(ticket.id, comment.trim(), noteMode);
      setComment("");
    });
  }

  async function createBill() {
    if (!ticket) return;
    await run(
      () => api.createPayable({ maintenance_ticket_id: ticket.id }),
      "Vendor bill drafted from this work order."
    );
    router.push("/console/payables");
  }

  return (
    <div className="space-y-6">
      <div>
        <Link href="/console/maintenance" className="text-sm text-ink-3">
          ← Back to maintenance
        </Link>
        <div className="mt-1 flex flex-wrap items-center gap-3">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            {ticket.title}
          </h1>
          <Badge tone={statusTone(ticket.status)}>
            {humanize(ticket.status)}
          </Badge>
          <Badge tone={ticket.priority === "urgent" ? "bad" : "neutral"}>
            {ticket.priority}
          </Badge>
        </div>
        <p className="text-ink-3">
          {ticket.category}
          {ticket.location ? ` · ${ticket.location}` : ""} · reported by{" "}
          {ticket.reporter ?? "—"} · {ticket.created_at.slice(0, 10)} ·{" "}
          <Link
            href={`/console/properties/${ticket.property_id}`}
            className="underline"
          >
            property
          </Link>
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {/* SLA panel */}
      <Card className="grid grid-cols-1 gap-4 p-5 sm:grid-cols-2">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-xs uppercase tracking-wide text-ink-3">
              First response
            </div>
            <div className="text-sm">
              due {fmtWhen(ticket.sla_response_due_at)} · responded{" "}
              {fmtWhen(ticket.first_response_at)}
            </div>
          </div>
          <Badge tone={slaTone(ticket.sla_response_state)}>
            {humanize(ticket.sla_response_state)}
          </Badge>
        </div>
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-xs uppercase tracking-wide text-ink-3">
              Resolution
            </div>
            <div className="text-sm">
              due {fmtWhen(ticket.sla_resolve_due_at)} · resolved{" "}
              {fmtWhen(ticket.resolved_at)}
            </div>
          </div>
          <Badge tone={slaTone(ticket.sla_resolve_state)}>
            {humanize(ticket.sla_resolve_state)}
          </Badge>
        </div>
      </Card>

      {/* Triage & dispatch */}
      <Card className="space-y-4 p-5">
        <h2 className="font-display text-lg font-bold">Triage & dispatch</h2>
        {ticket.description && (
          <p className="text-sm text-ink-2">{ticket.description}</p>
        )}
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <Badge tone={ticket.permission_to_enter ? "good" : "warn"}>
            {ticket.permission_to_enter
              ? "entry authorized"
              : "coordinate entry"}
          </Badge>
          {ticket.location && <Badge tone="neutral">{ticket.location}</Badge>}
          {ticket.asset_name && <Badge tone="info">{ticket.asset_name}</Badge>}
          {ticket.access_notes && (
            <span className="text-ink-3">Access: {ticket.access_notes}</span>
          )}
        </div>
        <div className="flex flex-wrap gap-4">
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Status
            <select
              className={select}
              value={ticket.status}
              disabled={!manage || busy}
              onChange={(e) =>
                void run(
                  () => api.updateTicket(ticket.id, { status: e.target.value }),
                  "Status updated."
                )
              }
            >
              {STATUSES.map((s) => (
                <option key={s} value={s}>
                  {humanize(s)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Priority
            <select
              className={select}
              value={ticket.priority}
              disabled={!manage || busy}
              onChange={(e) =>
                void run(
                  () =>
                    api.updateTicket(ticket.id, { priority: e.target.value }),
                  "Priority updated (SLA re-stamped)."
                )
              }
            >
              {PRIORITIES.map((p) => (
                <option key={p} value={p}>
                  {humanize(p)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Assigned member
            <select
              className={select}
              value={ticket.assignee_user_id ?? ""}
              disabled={!manage || busy}
              onChange={(e) => {
                if (!e.target.value) return;
                void run(
                  () =>
                    api.updateTicket(ticket.id, {
                      assignee_user_id: e.target.value,
                    }),
                  "Member assigned and notified."
                );
              }}
            >
              <option value="">Unassigned</option>
              {members.map((m) => (
                <option key={m.user_id} value={m.user_id}>
                  {m.name}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Contractor
            <select
              className={select}
              value={ticket.assignee_entity_id ?? ""}
              disabled={!manage || busy}
              onChange={(e) => {
                if (!e.target.value) return;
                void run(
                  () =>
                    api.updateTicket(ticket.id, {
                      assignee_entity_id: e.target.value,
                    }),
                  "Contractor dispatched."
                );
              }}
            >
              <option value="">None</option>
              {contractors.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Equipment
            <select
              className={select}
              value={ticket.asset_id ?? ""}
              disabled={!manage || busy}
              onChange={(e) => {
                if (!e.target.value) return;
                void run(
                  () =>
                    api.updateTicket(ticket.id, { asset_id: e.target.value }),
                  "Equipment attached."
                );
              }}
            >
              <option value="">None</option>
              {assets.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
            Scheduled date
            <input
              type="date"
              className={select}
              defaultValue={ticket.due_date ?? ""}
              disabled={!manage || busy}
              onBlur={(e) => {
                if (e.target.value && e.target.value !== ticket.due_date) {
                  void run(
                    () =>
                      api.updateTicket(ticket.id, { due_date: e.target.value }),
                    "Scheduled."
                  );
                }
              }}
            />
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-3 text-sm">
          <span className="text-ink-3">
            Cost: <span className="font-mono">{ticket.cost_label ?? "—"}</span>
          </span>
          {manage &&
            ticket.assignee_entity_id &&
            ticket.cost_cents != null &&
            ["resolved", "closed"].includes(ticket.status) && (
              <Button disabled={busy} onClick={() => void createBill()}>
                Create vendor bill
              </Button>
            )}
        </div>
      </Card>

      {/* Contractor quotes */}
      <QuotesCard
        ticket={ticket}
        contractors={contractors}
        manage={manage}
        approve={approve}
        busy={busy}
        run={run}
      />

      {/* Timeline */}
      <Card>
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Timeline
        </div>
        <div className="space-y-2 p-5">
          {ticket.comments.length === 0 && (
            <p className="text-sm text-ink-3">No activity yet.</p>
          )}
          {ticket.comments.map((c) => (
            <div
              key={c.id}
              className={
                c.visibility === "internal"
                  ? "rounded-xl border border-line-2 bg-warn-soft px-3 py-2 text-sm"
                  : "rounded-xl bg-surface-2 px-3 py-2 text-sm"
              }
            >
              <span className="text-xs text-ink-3">
                {c.author_name ?? c.kind} · {fmtWhen(c.created_at)}
                {c.visibility === "internal" && (
                  <span className="ml-2 font-semibold uppercase">internal</span>
                )}
              </span>
              <div className="whitespace-pre-wrap">{c.body}</div>
            </div>
          ))}
          {manage && (
            <form onSubmit={sendComment} className="flex gap-2 pt-2">
              <select
                className={select}
                value={noteMode}
                onChange={(e) =>
                  setNoteMode(e.target.value as "public" | "internal")
                }
              >
                <option value="public">Reply</option>
                <option value="internal">Internal note</option>
              </select>
              <input
                className={field}
                placeholder={
                  noteMode === "internal"
                    ? "Add an internal note (staff-only)…"
                    : "Reply to the resident…"
                }
                value={comment}
                onChange={(e) => setComment(e.target.value)}
              />
              <Button type="submit" disabled={busy || !comment.trim()}>
                {noteMode === "internal" ? "Add note" : "Send"}
              </Button>
            </form>
          )}
        </div>
      </Card>

      {/* Attachments (resident photos land here too) */}
      <DocumentsCard
        ownerType="maintenance_ticket"
        ownerId={ticket.id}
        title="Photos & attachments"
      />
    </div>
  );
}

function QuotesCard({
  ticket,
  contractors,
  manage,
  approve,
  busy,
  run,
}: {
  ticket: TicketDetail;
  contractors: Counterparty[];
  manage: boolean;
  approve: boolean;
  busy: boolean;
  run: (fn: () => Promise<unknown>, ok?: string) => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  const [entityId, setEntityId] = useState("");
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const cents = Math.round(parseFloat(amount) * 100);
    if (!description.trim() || !Number.isFinite(cents) || cents <= 0) {
      toast.error("A quote needs a description and a positive amount.");
      return;
    }
    await run(
      () =>
        api.addTicketQuote(ticket.id, {
          entity_id: entityId || undefined,
          description: description.trim(),
          amount_cents: cents,
        }),
      "Quote recorded."
    );
    setAdding(false);
    setDescription("");
    setAmount("");
    setEntityId("");
  }

  return (
    <Card>
      <div className="flex items-center justify-between border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">Quotes</h2>
        {manage && !adding && (
          <Button
            variant="outline"
            disabled={busy}
            onClick={() => setAdding(true)}
          >
            Record quote
          </Button>
        )}
      </div>
      <div className="space-y-3 p-5 text-sm">
        {adding && (
          <form onSubmit={submit} className="flex flex-wrap gap-2">
            <select
              className={select}
              value={entityId}
              onChange={(e) => setEntityId(e.target.value)}
            >
              <option value="">
                {ticket.assignee_entity_id
                  ? "Assigned contractor"
                  : "Pick a contractor…"}
              </option>
              {contractors.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            <input
              className={`${field} max-w-[280px]`}
              placeholder="Scope, e.g. “Replace faucet + labor”"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
            <input
              className={`${field} max-w-[120px]`}
              placeholder="0.00"
              inputMode="decimal"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
            />
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
        {ticket.quotes.length === 0 && !adding && (
          <p className="text-ink-3">
            No quotes yet — record one when a contractor bids the work.
          </p>
        )}
        {ticket.quotes.map((q) => (
          <div
            key={q.id}
            className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line px-4 py-3"
          >
            <div className="min-w-0">
              <div className="font-semibold">
                {q.entity_name ?? "Contractor"} ·{" "}
                <span className="font-mono">{q.amount_label}</span>
              </div>
              <div className="truncate text-xs text-ink-3">{q.description}</div>
            </div>
            <div className="flex items-center gap-2">
              <Badge tone={statusTone(q.status)}>{q.status}</Badge>
              {approve && q.status === "pending" && (
                <>
                  <Button
                    disabled={busy}
                    onClick={() =>
                      void run(
                        () => api.approveTicketQuote(q.id),
                        "Quote approved — amount set as the ticket cost."
                      )
                    }
                  >
                    Approve
                  </Button>
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() =>
                      void run(
                        () => api.rejectTicketQuote(q.id),
                        "Quote rejected."
                      )
                    }
                  >
                    Reject
                  </Button>
                </>
              )}
            </div>
          </div>
        ))}
      </div>
    </Card>
  );
}
