"use client";

// Leasing CRM: leads from the monitored leasing inbox (mail to that address
// creates/updates a lead automatically) or entered by hand. Work a prospect
// from first contact → tour → application without leaving the platform. Gated
// by `application:read`; working a lead needs `application:write`.

import { useState } from "react";
import Link from "next/link";
import { useAuth } from "@/lib/auth";
import {
  useConvertLead,
  useCreateLead,
  useLeads,
  useScheduleTour,
  useUpdateLead,
} from "@/lib/queries";
import type { Lead } from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

const STATUSES = ["new", "contacted", "toured", "applied", "closed"];
const SOURCES = ["manual", "website", "referral", "walk_in"];

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
  return key.charAt(0).toUpperCase() + key.slice(1).replace(/_/g, " ");
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
            Prospective renters, from first contact to signed application.
          </p>
        </div>
        <div className="flex items-end gap-3">
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
          {write && <NewLeadDialog />}
        </div>
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
        <div className="grid grid-cols-[1.6fr_.8fr_.7fr_.7fr_1.7fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Lead</span>
          <span>Phone</span>
          <span>Source</span>
          <span>Status</span>
          <span className="text-right">Actions</span>
        </div>
        <div className="divide-y divide-line">
          {leads?.map((l) => (
            <div
              key={l.id}
              className="grid grid-cols-[1.6fr_.8fr_.7fr_.7fr_1.7fr] items-center gap-4 px-5 py-3.5"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">{l.name}</div>
                <div className="truncate text-sm text-ink-3">{l.email}</div>
                {l.last_message && (
                  <div className="mt-0.5 line-clamp-1 text-xs text-ink-3">
                    {l.last_message}
                  </div>
                )}
              </div>
              <span className="truncate text-sm text-ink-2">
                {l.phone ?? "—"}
              </span>
              <span className="truncate text-sm text-ink-2">
                {humanize(l.source)}
              </span>
              <span>
                <Badge tone={leadTone(l.status)}>{l.status}</Badge>
              </span>
              <span className="flex items-center justify-end gap-2">
                {write ? (
                  <>
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
                    <TourDialog lead={l} />
                    {l.application_id ? (
                      <Link
                        href="/console/applications"
                        className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-accent-2 hover:bg-surface-2"
                      >
                        View app
                      </Link>
                    ) : (
                      <ConvertDialog lead={l} />
                    )}
                  </>
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
              No leads{status ? " in this status" : ""} yet — add one, or they
              arrive automatically from the leasing inbox.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

function NewLeadDialog() {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [source, setSource] = useState("manual");
  const [notes, setNotes] = useState("");
  const create = useCreateLead();

  function reset() {
    setName("");
    setEmail("");
    setPhone("");
    setSource("manual");
    setNotes("");
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    await create.mutateAsync(
      {
        name: name.trim(),
        email: email.trim(),
        phone: phone.trim() || undefined,
        source,
        notes: notes.trim() || undefined,
      },
      {
        onSuccess: () => {
          reset();
          setOpen(false);
        },
      }
    );
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>New lead</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>New lead</DialogTitle>
            <DialogDescription>
              Enter a prospect who reached you off-platform (walk-in, phone,
              referral).
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Name</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} required />
            </div>
            <div className="space-y-1.5">
              <Label>Email</Label>
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Phone</Label>
                <Input value={phone} onChange={(e) => setPhone(e.target.value)} />
              </div>
              <div className="space-y-1.5">
                <Label>Source</Label>
                <select
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                  className="h-9 w-full rounded-lg border border-line bg-surface px-3 text-sm text-ink"
                >
                  {SOURCES.map((s) => (
                    <option key={s} value={s}>
                      {humanize(s)}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="space-y-1.5">
              <Label>Notes</Label>
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                className="w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm text-ink"
              />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={create.isPending}>
              {create.isPending ? "Creating…" : "Create lead"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function TourDialog({ lead }: { lead: Lead }) {
  const [open, setOpen] = useState(false);
  const [date, setDate] = useState("");
  const [notes, setNotes] = useState("");
  const tour = useScheduleTour();

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    await tour.mutateAsync(
      { id: lead.id, body: { date, notes: notes.trim() || undefined } },
      {
        onSuccess: () => {
          setDate("");
          setNotes("");
          setOpen(false);
        },
      }
    );
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:bg-surface-2">
          Tour
        </button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Schedule a tour</DialogTitle>
            <DialogDescription>
              Books a showing for {lead.name} on the calendar and moves the lead
              into the pipeline. Staff are reminded ahead of the date.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Tour date</Label>
              <Input
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
                required
              />
            </div>
            <div className="space-y-1.5">
              <Label>Notes</Label>
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                placeholder="Which unit, meeting point, etc."
                className="w-full rounded-lg border border-line bg-surface px-3 py-2 text-sm text-ink"
              />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={tour.isPending || !date}>
              {tour.isPending ? "Scheduling…" : "Schedule tour"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function ConvertDialog({ lead }: { lead: Lead }) {
  const [open, setOpen] = useState(false);
  const [moveIn, setMoveIn] = useState("");
  const [income, setIncome] = useState("");
  const [consent, setConsent] = useState(true);
  const convert = useConvertLead();

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const dollars = parseFloat(income);
    await convert.mutateAsync(
      {
        id: lead.id,
        body: {
          move_in: moveIn || undefined,
          annual_income_cents:
            income && !Number.isNaN(dollars)
              ? Math.round(dollars * 100)
              : undefined,
          screening_consent: consent,
        },
      },
      {
        onSuccess: () => {
          setMoveIn("");
          setIncome("");
          setOpen(false);
        },
      }
    );
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button className="rounded-lg border border-accent px-2 py-1 text-xs font-semibold text-accent-2 hover:bg-accent-soft">
          Convert
        </button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Convert to application</DialogTitle>
            <DialogDescription>
              Starts a rental application for {lead.name} ({lead.email}) — it
              enters screening like any other, and the lead is marked applied.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Desired move-in</Label>
                <Input
                  type="date"
                  value={moveIn}
                  onChange={(e) => setMoveIn(e.target.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label>Annual income ($)</Label>
                <Input
                  type="number"
                  min="0"
                  step="1000"
                  value={income}
                  onChange={(e) => setIncome(e.target.value)}
                />
              </div>
            </div>
            <label className="flex items-start gap-2 text-sm text-ink-2">
              <input
                type="checkbox"
                checked={consent}
                onChange={(e) => setConsent(e.target.checked)}
                className="mt-0.5"
              />
              <span>
                The applicant authorized a consumer report (credit, criminal,
                eviction) — FCRA §604(b). Required to screen.
              </span>
            </label>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={convert.isPending || !consent}>
              {convert.isPending ? "Converting…" : "Convert to application"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
