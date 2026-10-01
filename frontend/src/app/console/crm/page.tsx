"use client";

// Owner CRM: the people whose properties we manage (owners) and the people who
// might hire us (owner leads). Follow-ups that are due, an owner directory
// with each owner's timeline, and a pipeline board from first contact to a
// signed management agreement. Gated by `entity:read`; changes need
// `entity:manage`.

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { ChevronLeft, ChevronRight, Printer, UserPlus } from "lucide-react";
import {
  crm,
  isoDate,
  LEAD_SOURCES,
  LEAD_STATUSES,
  money,
  openPdf,
  pct,
  toCents,
  type CrmNote,
  type LeadStatus,
  type OwnerLead,
  type OwnerLeadInput,
  type OwnerRow,
  type PipelineSummary,
  type SubjectType,
} from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  CrmTimeline,
  KindBadge,
  followUpState,
  parseDay,
  prettyDay,
} from "@/components/CrmTimeline";

type Tab = "followups" | "owners" | "pipeline";
type Opened = { type: SubjectType; id: string; name: string | null };

const field =
  "w-full rounded-xl border border-line bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent";
const selectCls =
  "h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm text-ink outline-none focus:border-accent";

const OWNER_KINDS = ["individual", "company", "firm"] as const;

const SUBJECT_LABEL: Record<SubjectType, string> = {
  owner: "Owner",
  owner_lead: "Lead",
  counterparty: "Vendor",
  property: "Property",
};

const STAGE_LABEL: Record<LeadStatus, string> = {
  new: "New",
  contacted: "Contacted",
  proposal: "Proposal sent",
  won: "Won",
  lost: "Lost",
};

function stageTone(s: LeadStatus): "info" | "accent" | "warn" | "good" | "bad" {
  switch (s) {
    case "new":
      return "info";
    case "contacted":
      return "accent";
    case "proposal":
      return "warn";
    case "won":
      return "good";
    default:
      return "bad";
  }
}

function humanize(key: string): string {
  return key.charAt(0).toUpperCase() + key.slice(1).replace(/_/g, " ");
}

function errMsg(e: unknown, fallback: string) {
  return e instanceof Error ? e.message : fallback;
}

/** "3 days ago" for an instant. */
function ago(iso: string | null): string {
  if (!iso) return "Never";
  const s = Math.max(
    0,
    (new Date().getTime() - new Date(iso).getTime()) / 1000
  );
  if (s < 60) return "Just now";
  const units: [number, string][] = [
    [60 * 60 * 24 * 365, "year"],
    [60 * 60 * 24 * 30, "month"],
    [60 * 60 * 24 * 7, "week"],
    [60 * 60 * 24, "day"],
    [60 * 60, "hour"],
    [60, "minute"],
  ];
  for (const [size, name] of units) {
    const n = Math.floor(s / size);
    if (n >= 1) return `${n} ${name}${n === 1 ? "" : "s"} ago`;
  }
  return "Just now";
}

/** `YYYY-MM-DD` `days` after the later of today and `from`. */
function snoozeDay(from: string | null, days: number): string {
  const today = isoDate(new Date());
  const base = parseDay(from && from > today ? from : today);
  base.setDate(base.getDate() + days);
  return isoDate(base);
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export default function CrmPage() {
  const { can } = useAuth();
  const read = can("entity:read");
  const manage = can("entity:manage");

  const [tab, setTab] = useState<Tab | null>(null);
  const [due, setDue] = useState<CrmNote[] | null>(null);
  const [owners, setOwners] = useState<OwnerRow[] | null>(null);
  const [leads, setLeads] = useState<OwnerLead[] | null>(null);
  const [summary, setSummary] = useState<PipelineSummary | null>(null);
  const [opened, setOpened] = useState<Opened | null>(null);
  const [lostFor, setLostFor] = useState<OwnerLead | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadDue = useCallback(() => {
    crm
      .followUps()
      .then(setDue)
      .catch((e) => setError(e.message));
  }, []);
  const loadOwners = useCallback(() => {
    crm
      .owners()
      .then(setOwners)
      .catch((e) => setError(e.message));
  }, []);
  const loadPipeline = useCallback(() => {
    crm
      .leads()
      .then(setLeads)
      .catch((e) => setError(e.message));
    crm
      .pipeline()
      .then(setSummary)
      .catch(() => undefined);
  }, []);
  const refreshAll = useCallback(() => {
    loadDue();
    loadOwners();
    loadPipeline();
  }, [loadDue, loadOwners, loadPipeline]);

  useEffect(() => {
    if (!read) return;
    refreshAll();
  }, [read, refreshAll]);

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to the owner CRM. Ask an admin for the{" "}
          <span className="font-mono">entity:read</span> permission.
        </p>
      </Card>
    );
  }

  // Land on follow-ups when something is due, otherwise on owners.
  const active: Tab | null =
    tab ?? (due === null ? null : due.length ? "followups" : "owners");

  async function moveLead(l: OwnerLead, status: LeadStatus, reason?: string) {
    if (status === l.status) return;
    if (status === "lost" && reason === undefined) {
      setLostFor(l);
      return;
    }
    try {
      await crm.updateLead(l.id, {
        status,
        ...(status === "lost" ? { lost_reason: reason ?? "" } : {}),
      });
      toast.success(`${l.name} moved to ${STAGE_LABEL[status]}`);
      loadPipeline();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't move the lead"));
    }
  }

  const tabs: { key: Tab; label: string; count?: number }[] = [
    { key: "followups", label: "Follow-ups", count: due?.length },
    { key: "owners", label: "Owners", count: owners?.length },
    { key: "pipeline", label: "Pipeline", count: leads?.length },
  ];

  const openOwner =
    opened?.type === "owner"
      ? owners?.find((o) => o.id === opened.id)
      : undefined;
  const openLead =
    opened?.type === "owner_lead"
      ? leads?.find((l) => l.id === opened.id)
      : undefined;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Owner CRM
          </h1>
          <p className="text-ink-3">
            Keep up with the owners you manage for, and win new ones.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {tabs.map((t) => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-semibold ${
                active === t.key
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-3 hover:bg-surface-2"
              }`}
            >
              {t.label}
              {t.count !== undefined && t.count > 0 && (
                <span
                  className={`rounded-full px-1.5 text-xs ${
                    t.key === "followups"
                      ? "bg-warn-soft text-warn"
                      : "bg-surface-2 text-ink-3"
                  }`}
                >
                  {t.count}
                </span>
              )}
            </button>
          ))}
        </div>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {active === null && (
        <Card className="px-5 py-10 text-center text-ink-3">Loading…</Card>
      )}
      {active === "followups" && (
        <FollowUpsTab
          due={due}
          manage={manage}
          onChanged={refreshAll}
          onOpen={setOpened}
        />
      )}
      {active === "owners" && (
        <OwnersTab
          owners={owners}
          manage={manage}
          onCreated={(o) => {
            loadOwners();
            setOpened({ type: "owner", id: o.id, name: o.name });
          }}
          onOpen={setOpened}
        />
      )}
      {active === "pipeline" && (
        <PipelineTab
          leads={leads}
          summary={summary}
          manage={manage}
          onMove={moveLead}
          onCreated={(l) => {
            loadPipeline();
            setOpened({ type: "owner_lead", id: l.id, name: l.name });
          }}
          onOpen={setOpened}
        />
      )}

      <Drawer
        open={opened !== null}
        onClose={() => setOpened(null)}
        title={openOwner?.name ?? openLead?.name ?? opened?.name ?? "Timeline"}
        subtitle={opened ? SUBJECT_LABEL[opened.type] : ""}
      >
        {opened && opened.type === "owner" && openOwner && (
          <OwnerDetails
            key={openOwner.id}
            owner={openOwner}
            manage={manage}
            onSaved={loadOwners}
          />
        )}
        {opened && opened.type === "owner_lead" && openLead && (
          <LeadDetails
            key={openLead.id}
            lead={openLead}
            manage={manage}
            onSaved={loadPipeline}
            onMove={(s) => moveLead(openLead, s)}
            onConverted={() => {
              setOpened(null);
              loadPipeline();
              loadOwners();
              setTab("owners");
            }}
          />
        )}
        {opened && (
          <section className="space-y-3">
            <h3 className="font-display text-lg font-bold">Timeline</h3>
            <CrmTimeline
              key={`${opened.type}:${opened.id}`}
              subjectType={opened.type}
              subjectId={opened.id}
              canManage={manage}
              onChange={refreshAll}
            />
          </section>
        )}
      </Drawer>

      <LostReasonDialog
        lead={lostFor}
        onClose={() => setLostFor(null)}
        onConfirm={(reason) => {
          const l = lostFor;
          setLostFor(null);
          if (l) void moveLead(l, "lost", reason);
        }}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Drawer (a right-hand panel built on the dialog primitive)
// ---------------------------------------------------------------------------

function Drawer({
  open,
  onClose,
  title,
  subtitle,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="left-auto right-0 top-0 h-dvh max-w-2xl translate-x-0 translate-y-0 content-start gap-6 overflow-y-auto sm:rounded-none sm:rounded-l-2xl">
        <DialogHeader>
          <span className="text-xs font-bold uppercase tracking-wide text-ink-3">
            {subtitle}
          </span>
          <DialogTitle className="text-2xl">{title}</DialogTitle>
          <DialogDescription className="sr-only">
            Details and timeline
          </DialogDescription>
        </DialogHeader>
        {children}
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Follow-ups
// ---------------------------------------------------------------------------

function FollowUpsTab({
  due,
  manage,
  onChanged,
  onOpen,
}: {
  due: CrmNote[] | null;
  manage: boolean;
  onChanged: () => void;
  onOpen: (o: Opened) => void;
}) {
  const [upcoming, setUpcoming] = useState<CrmNote[] | null>(null);
  const [showUpcoming, setShowUpcoming] = useState(false);

  const loadUpcoming = useCallback(() => {
    crm
      .followUps(true)
      .then(setUpcoming)
      .catch((e) => toast.error(errMsg(e, "Couldn't load follow-ups")));
  }, []);

  function toggle(on: boolean) {
    setShowUpcoming(on);
    if (on) loadUpcoming();
  }

  function changed() {
    onChanged();
    if (showUpcoming) loadUpcoming();
  }

  async function patch(
    n: CrmNote,
    body: Parameters<typeof crm.updateNote>[1],
    ok: string
  ) {
    try {
      await crm.updateNote(n.id, body);
      toast.success(ok);
      changed();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't update the follow-up"));
    }
  }

  const items = showUpcoming ? upcoming : due;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        {[
          { on: false, label: "Due now" },
          { on: true, label: "Include upcoming" },
        ].map((o) => (
          <button
            key={o.label}
            onClick={() => toggle(o.on)}
            className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
              showUpcoming === o.on
                ? "bg-accent-soft text-accent-2"
                : "text-ink-3 hover:bg-surface-2"
            }`}
          >
            {o.label}
          </button>
        ))}
      </div>

      <Card className="overflow-hidden">
        <div className="divide-y divide-line">
          {items?.map((n) => {
            const fu = n.follow_up_on ? followUpState(n.follow_up_on) : null;
            return (
              <div
                key={n.id}
                className="flex flex-wrap items-start gap-4 px-5 py-4"
              >
                <div className="min-w-0 flex-1 space-y-1.5">
                  <div className="flex flex-wrap items-center gap-2">
                    <button
                      onClick={() =>
                        onOpen({
                          type: n.subject_type,
                          id: n.subject_id,
                          name: n.subject_name,
                        })
                      }
                      className="truncate font-semibold hover:text-accent-2 hover:underline"
                    >
                      {n.subject_name ?? "Unknown"}
                    </button>
                    <Badge className="px-2 py-0.5">
                      {SUBJECT_LABEL[n.subject_type]}
                    </Badge>
                    <KindBadge kind={n.kind} />
                  </div>
                  <p className="line-clamp-2 whitespace-pre-wrap text-sm text-ink-2">
                    {n.body}
                  </p>
                  <p className="text-xs text-ink-3">
                    {n.author ?? "System"} ·{" "}
                    {new Date(n.created_at).toLocaleDateString([], {
                      month: "short",
                      day: "numeric",
                    })}
                  </p>
                </div>
                <div className="flex flex-col items-end gap-2">
                  {fu && <Badge tone={fu.tone}>{fu.label}</Badge>}
                  <div className="flex flex-wrap justify-end gap-1.5">
                    {manage && (
                      <>
                        <SmallButton
                          onClick={() =>
                            patch(n, { follow_up_done: true }, "Marked done")
                          }
                        >
                          Done
                        </SmallButton>
                        <SmallButton
                          onClick={() => {
                            const d = snoozeDay(n.follow_up_on, 1);
                            patch(
                              n,
                              { follow_up_on: d },
                              `Snoozed to ${prettyDay(d)}`
                            );
                          }}
                        >
                          +1 day
                        </SmallButton>
                        <SmallButton
                          onClick={() => {
                            const d = snoozeDay(n.follow_up_on, 7);
                            patch(
                              n,
                              { follow_up_on: d },
                              `Snoozed to ${prettyDay(d)}`
                            );
                          }}
                        >
                          +1 week
                        </SmallButton>
                      </>
                    )}
                    <SmallButton
                      accent
                      onClick={() =>
                        onOpen({
                          type: n.subject_type,
                          id: n.subject_id,
                          name: n.subject_name,
                        })
                      }
                    >
                      Open
                    </SmallButton>
                  </div>
                </div>
              </div>
            );
          })}
          {items === null && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {items && items.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              {showUpcoming
                ? "No open follow-ups. Add one from an owner's or lead's timeline."
                : "You're all caught up — nothing is due today."}
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

function SmallButton({
  accent,
  className,
  ...rest
}: { accent?: boolean } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      {...rest}
      className={`rounded-lg border px-2 py-1 text-xs font-semibold disabled:opacity-50 ${
        accent
          ? "border-accent text-accent-2 hover:bg-accent-soft"
          : "border-line text-ink-2 hover:bg-surface-2"
      } ${className ?? ""}`}
    />
  );
}

// ---------------------------------------------------------------------------
// Owners
// ---------------------------------------------------------------------------

const OWNER_COLS =
  "grid grid-cols-[1.4fr_1.3fr_1.2fr_.6fr_.5fr_.9fr_.9fr] gap-4";

function OwnersTab({
  owners,
  manage,
  onCreated,
  onOpen,
}: {
  owners: OwnerRow[] | null;
  manage: boolean;
  onCreated: (o: { id: string; name: string }) => void;
  onOpen: (o: Opened) => void;
}) {
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const q = query.trim().toLowerCase();
  const rows = owners?.filter(
    (o) =>
      !q ||
      o.name.toLowerCase().includes(q) ||
      (o.email ?? "").toLowerCase().includes(q) ||
      o.entities.some((e) => e.toLowerCase().includes(q))
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search owners, emails or LLCs"
          className="w-full max-w-xs rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent"
        />
        {manage && <Button onClick={() => setCreating(true)}>New owner</Button>}
      </div>

      <Card className="overflow-x-auto">
        <div className="min-w-[900px]">
          <div
            className={`${OWNER_COLS} border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3`}
          >
            <span>Owner</span>
            <span>Contact</span>
            <span>Entities</span>
            <span className="text-right">Properties</span>
            <span className="text-right">Doors</span>
            <span>Last contact</span>
            <span className="text-right">Follow-ups</span>
          </div>
          <div className="divide-y divide-line">
            {rows?.map((o) => (
              <button
                key={o.id}
                onClick={() =>
                  onOpen({ type: "owner", id: o.id, name: o.name })
                }
                className={`${OWNER_COLS} w-full items-center px-5 py-3.5 text-left hover:bg-surface-2`}
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold">{o.name}</div>
                  <div className="text-xs text-ink-3">{humanize(o.kind)}</div>
                </div>
                <div className="min-w-0 text-sm text-ink-2">
                  <div className="truncate">{o.email ?? "—"}</div>
                  {o.phone && (
                    <div className="truncate text-xs text-ink-3">{o.phone}</div>
                  )}
                </div>
                <div className="min-w-0 truncate text-sm text-ink-2">
                  {o.entities.length ? o.entities.join(", ") : "—"}
                </div>
                <span className="text-right text-sm">{o.properties}</span>
                <span className="text-right text-sm">{o.doors}</span>
                <span className="text-sm text-ink-2">
                  {ago(o.last_contact_at)}
                </span>
                <span className="flex justify-end gap-1.5">
                  {o.follow_ups_due > 0 && (
                    <Badge tone="bad" className="px-2 py-0.5">
                      {o.follow_ups_due} due
                    </Badge>
                  )}
                  {o.open_follow_ups > o.follow_ups_due && (
                    <Badge className="px-2 py-0.5">
                      {o.open_follow_ups - o.follow_ups_due} open
                    </Badge>
                  )}
                  {o.open_follow_ups === 0 && (
                    <span className="text-sm text-ink-3">—</span>
                  )}
                </span>
              </button>
            ))}
            {owners === null && (
              <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
            )}
            {rows && rows.length === 0 && (
              <div className="px-5 py-10 text-center text-ink-3">
                {q
                  ? "No owners match that search."
                  : "No owners yet — add one, or win a lead in the pipeline."}
              </div>
            )}
          </div>
        </div>
      </Card>

      <OwnerDialog
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(o) => {
          setCreating(false);
          onCreated(o);
        }}
      />
    </div>
  );
}

function OwnerDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (o: { id: string; name: string }) => void;
}) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<string>("individual");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      const o = await crm.createOwner({
        name: name.trim(),
        kind,
        email: email.trim() || undefined,
        phone: phone.trim() || undefined,
        notes: notes.trim() || undefined,
      });
      toast.success(`Added ${o.name}`);
      setName("");
      setKind("individual");
      setEmail("");
      setPhone("");
      setNotes("");
      onCreated(o);
    } catch (err) {
      toast.error(errMsg(err, "Couldn't add the owner"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>New owner</DialogTitle>
            <DialogDescription>
              Someone whose properties you manage. You can link their LLCs and
              properties from Entities.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="grid grid-cols-[1fr_auto] gap-3">
              <div className="space-y-1.5">
                <Label>Name</Label>
                <Input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                />
              </div>
              <div className="space-y-1.5">
                <Label>Type</Label>
                <select
                  value={kind}
                  onChange={(e) => setKind(e.target.value)}
                  className={selectCls}
                >
                  {OWNER_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {humanize(k)}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Email</Label>
                <Input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label>Phone</Label>
                <Input
                  value={phone}
                  onChange={(e) => setPhone(e.target.value)}
                />
              </div>
            </div>
            <div className="space-y-1.5">
              <Label>Notes</Label>
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                rows={2}
                className={field}
              />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !name.trim()}>
              {busy ? "Adding…" : "Add owner"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function OwnerDetails({
  owner,
  manage,
  onSaved,
}: {
  owner: OwnerRow;
  manage: boolean;
  onSaved: () => void;
}) {
  const [name, setName] = useState(owner.name);
  const [kind, setKind] = useState(owner.kind);
  const [email, setEmail] = useState(owner.email ?? "");
  const [phone, setPhone] = useState(owner.phone ?? "");
  const [notes, setNotes] = useState(owner.notes ?? "");
  const [busy, setBusy] = useState(false);

  const dirty =
    name !== owner.name ||
    kind !== owner.kind ||
    email !== (owner.email ?? "") ||
    phone !== (owner.phone ?? "") ||
    notes !== (owner.notes ?? "");

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    try {
      await crm.updateOwner(owner.id, {
        name: name.trim(),
        kind,
        email: email.trim(),
        phone: phone.trim(),
        notes: notes.trim(),
      });
      toast.success("Owner saved");
      onSaved();
    } catch (err) {
      toast.error(errMsg(err, "Couldn't save the owner"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-4">
      <div className="grid grid-cols-3 gap-3">
        <Stat label="Properties" value={String(owner.properties)} />
        <Stat label="Doors" value={String(owner.doors)} />
        <Stat label="Last contact" value={ago(owner.last_contact_at)} />
      </div>
      {owner.entities.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs font-semibold text-ink-3">Entities</span>
          {owner.entities.map((e) => (
            <Badge key={e} className="px-2 py-0.5">
              {e}
            </Badge>
          ))}
        </div>
      )}
      <form onSubmit={save} className="space-y-3">
        <fieldset disabled={!manage} className="space-y-3">
          <div className="grid grid-cols-[1fr_auto] gap-3">
            <div className="space-y-1.5">
              <Label>Name</Label>
              <Input value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="space-y-1.5">
              <Label>Type</Label>
              <select
                value={kind}
                onChange={(e) => setKind(e.target.value)}
                className={selectCls}
              >
                {OWNER_KINDS.map((k) => (
                  <option key={k} value={k}>
                    {humanize(k)}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label>Email</Label>
              <Input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Phone</Label>
              <Input value={phone} onChange={(e) => setPhone(e.target.value)} />
            </div>
          </div>
          <div className="space-y-1.5">
            <Label>Notes</Label>
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={2}
              className={field}
            />
          </div>
        </fieldset>
        {manage && (
          <div className="flex justify-end">
            <Button type="submit" disabled={busy || !dirty || !name.trim()}>
              {busy ? "Saving…" : "Save changes"}
            </Button>
          </div>
        )}
      </form>
    </section>
  );
}

function Stat({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="rounded-xl border border-line bg-surface-2 px-3 py-2.5">
      <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </div>
      <div className="font-display text-lg font-extrabold tracking-tight">
        {value}
      </div>
      {sub && <div className="text-xs text-ink-3">{sub}</div>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

function PipelineTab({
  leads,
  summary,
  manage,
  onMove,
  onCreated,
  onOpen,
}: {
  leads: OwnerLead[] | null;
  summary: PipelineSummary | null;
  manage: boolean;
  onMove: (l: OwnerLead, s: LeadStatus) => void;
  onCreated: (l: OwnerLead) => void;
  onOpen: (o: Opened) => void;
}) {
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-5">
      {summary && <SummaryStrip summary={summary} />}

      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-ink-3">
          Owners who might hire you to manage their properties.
        </p>
        {manage && <Button onClick={() => setCreating(true)}>New lead</Button>}
      </div>

      {leads === null ? (
        <Card className="px-5 py-10 text-center text-ink-3">Loading…</Card>
      ) : (
        <div className="overflow-x-auto pb-2">
          <div className="grid min-w-[1000px] grid-cols-5 gap-3">
            {LEAD_STATUSES.map((stage) => {
              const cards = leads.filter((l) => l.status === stage);
              return (
                <div
                  key={stage}
                  className="flex flex-col gap-2 rounded-2xl border border-line bg-surface-2/60 p-2.5"
                >
                  <div className="flex items-center justify-between px-1">
                    <Badge tone={stageTone(stage)} className="px-2 py-0.5">
                      {STAGE_LABEL[stage]}
                    </Badge>
                    <span className="text-xs font-semibold text-ink-3">
                      {cards.length}
                    </span>
                  </div>
                  {cards.map((l) => (
                    <LeadCard
                      key={l.id}
                      lead={l}
                      manage={manage}
                      onMove={(s) => onMove(l, s)}
                      onOpen={() =>
                        onOpen({ type: "owner_lead", id: l.id, name: l.name })
                      }
                    />
                  ))}
                  {cards.length === 0 && (
                    <div className="rounded-xl border border-dashed border-line px-3 py-6 text-center text-xs text-ink-3">
                      No leads here
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      <NewLeadDialog
        open={creating}
        onClose={() => setCreating(false)}
        onCreated={(l) => {
          setCreating(false);
          onCreated(l);
        }}
      />
    </div>
  );
}

function SummaryStrip({ summary }: { summary: PipelineSummary }) {
  return (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-5">
        {LEAD_STATUSES.map((s) => {
          const st = summary.stages.find((x) => x.status === s);
          return (
            <Card key={s} className="p-4">
              <div className="mb-1 flex items-center justify-between">
                <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
                  {STAGE_LABEL[s]}
                </span>
                <span className="font-display text-xl font-extrabold">
                  {st?.leads ?? 0}
                </span>
              </div>
              <div className="text-sm text-ink-2">
                {st?.doors ?? 0} doors ·{" "}
                <span className="font-semibold">
                  {money(st?.monthly_fee_cents ?? 0)}
                </span>
                /mo
              </div>
            </Card>
          );
        })}
      </div>
      <Card className="flex flex-wrap items-center gap-x-8 gap-y-3 px-5 py-4">
        <div>
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            Weighted monthly fees
          </div>
          <div className="font-display text-2xl font-extrabold tracking-tight">
            {money(summary.weighted_monthly_fee_cents)}
          </div>
        </div>
        <div>
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            Win rate
          </div>
          <div className="font-display text-2xl font-extrabold tracking-tight">
            {pct(summary.win_bps)}
          </div>
        </div>
        {summary.follow_ups_due > 0 && (
          <Badge tone="warn">{summary.follow_ups_due} follow-ups due</Badge>
        )}
        {summary.by_source.length > 0 && (
          <div className="flex flex-1 flex-wrap items-center justify-end gap-1.5">
            <span className="text-xs font-semibold text-ink-3">
              Win rate by source
            </span>
            {summary.by_source.map((s) => (
              <Badge key={s.source} className="px-2 py-0.5">
                {humanize(s.source)} · {pct(s.win_bps)}{" "}
                <span className="font-normal text-ink-3">
                  ({s.won}/{s.leads})
                </span>
              </Badge>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}

function LeadCard({
  lead: l,
  manage,
  onMove,
  onOpen,
}: {
  lead: OwnerLead;
  manage: boolean;
  onMove: (s: LeadStatus) => void;
  onOpen: () => void;
}) {
  const idx = LEAD_STATUSES.indexOf(l.status);
  const fu = l.next_follow_up ? followUpState(l.next_follow_up) : null;
  return (
    <div className="rounded-xl border border-line bg-surface shadow-acre">
      <button onClick={onOpen} className="w-full space-y-1.5 p-3 text-left">
        <div className="font-semibold leading-tight">{l.name}</div>
        {l.company && <div className="text-xs text-ink-3">{l.company}</div>}
        <div className="text-xs text-ink-2">
          {l.doors} doors · {l.properties_count}{" "}
          {l.properties_count === 1 ? "property" : "properties"}
        </div>
        <div className="text-sm">
          <span className="font-semibold">{money(l.monthly_fee_cents)}</span>
          <span className="text-ink-3">/mo · {pct(l.fee_bps)}</span>
        </div>
        <div className="flex flex-wrap items-center gap-1 text-xs text-ink-3">
          <span>{humanize(l.source)}</span>
          {l.assigned_name && <span>· {l.assigned_name}</span>}
        </div>
        {fu && (
          <Badge tone={fu.tone} className="px-2 py-0.5">
            Follow up · {fu.label}
          </Badge>
        )}
        {l.status === "lost" && l.lost_reason && (
          <div className="line-clamp-2 text-xs text-ink-3">
            Lost: {l.lost_reason}
          </div>
        )}
        {l.owner_id && (
          <Badge tone="good" className="px-2 py-0.5">
            Now an owner
          </Badge>
        )}
      </button>
      {manage && (
        <div className="flex items-center gap-1 border-t border-line px-2 py-1.5">
          <button
            disabled={idx <= 0}
            onClick={() => onMove(LEAD_STATUSES[idx - 1])}
            className="rounded-lg p-1 text-ink-3 hover:bg-surface-2 hover:text-ink disabled:opacity-30"
            aria-label="Move back a stage"
            title="Move back a stage"
          >
            <ChevronLeft size={16} />
          </button>
          <select
            value={l.status}
            onChange={(e) => onMove(e.target.value as LeadStatus)}
            className="min-w-0 flex-1 rounded-lg border border-line bg-surface px-1.5 py-1 text-xs text-ink"
            aria-label="Stage"
          >
            {LEAD_STATUSES.map((s) => (
              <option key={s} value={s}>
                {STAGE_LABEL[s]}
              </option>
            ))}
          </select>
          <button
            disabled={idx >= LEAD_STATUSES.length - 1}
            onClick={() => onMove(LEAD_STATUSES[idx + 1])}
            className="rounded-lg p-1 text-ink-3 hover:bg-surface-2 hover:text-ink disabled:opacity-30"
            aria-label="Move forward a stage"
            title="Move forward a stage"
          >
            <ChevronRight size={16} />
          </button>
        </div>
      )}
    </div>
  );
}

function LostReasonDialog({
  lead,
  onClose,
  onConfirm,
}: {
  lead: OwnerLead | null;
  onClose: () => void;
  onConfirm: (reason: string) => void;
}) {
  const [reason, setReason] = useState("");
  return (
    <Dialog
      open={lead !== null}
      onOpenChange={(o) => {
        if (!o) {
          setReason("");
          onClose();
        }
      }}
    >
      <DialogContent>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onConfirm(reason.trim());
            setReason("");
          }}
        >
          <DialogHeader>
            <DialogTitle>Mark as lost</DialogTitle>
            <DialogDescription>
              Why didn&apos;t {lead?.name ?? "this lead"} sign? It helps spot
              patterns later.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-1.5">
            <Label>Reason</Label>
            <textarea
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              rows={3}
              placeholder="e.g. Went with another manager, fee too high, decided to sell"
              className={field}
              autoFocus
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                setReason("");
                onClose();
              }}
            >
              Cancel
            </Button>
            <Button type="submit">Mark as lost</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

// ---- Lead form ---------------------------------------------------------------

type LeadForm = {
  name: string;
  company: string;
  email: string;
  phone: string;
  address: string;
  properties: string;
  doors: string;
  rent: string;
  fee: string;
  source: string;
  notes: string;
};

const EMPTY_LEAD: LeadForm = {
  name: "",
  company: "",
  email: "",
  phone: "",
  address: "",
  properties: "",
  doors: "",
  rent: "",
  fee: "",
  source: "referral",
  notes: "",
};

function leadToForm(l: OwnerLead): LeadForm {
  return {
    name: l.name,
    company: l.company ?? "",
    email: l.email ?? "",
    phone: l.phone ?? "",
    address: l.address ?? "",
    properties: String(l.properties_count),
    doors: String(l.doors),
    rent: l.monthly_rent_cents ? (l.monthly_rent_cents / 100).toFixed(2) : "",
    fee: (l.fee_bps / 100).toString(),
    source: l.source,
    notes: l.notes ?? "",
  };
}

/** Form → API body. Returns an error string when something doesn't parse. */
function formToInput(
  f: LeadForm,
  initialFee?: string
): OwnerLeadInput | string {
  const int = (v: string) => (v.trim() === "" ? undefined : Number(v));
  const properties = int(f.properties);
  const doors = int(f.doors);
  if (
    (properties !== undefined && !Number.isInteger(properties)) ||
    (doors !== undefined && !Number.isInteger(doors))
  )
    return "Properties and doors should be whole numbers.";
  const rent = f.rent.trim() ? toCents(f.rent) : null;
  if (f.rent.trim() && rent === null)
    return "Monthly rent should be a dollar amount.";
  let fee_bps: number | undefined;
  if (f.fee.trim() !== "" && f.fee.trim() !== initialFee) {
    const n = Number(f.fee.replace("%", ""));
    if (!Number.isFinite(n) || n < 0 || n > 50)
      return "Fee should be a percent between 0 and 50.";
    fee_bps = Math.round(n * 100);
  }
  return {
    name: f.name.trim(),
    company: f.company.trim(),
    email: f.email.trim(),
    phone: f.phone.trim(),
    address: f.address.trim(),
    source: f.source as OwnerLead["source"],
    notes: f.notes.trim(),
    ...(properties !== undefined ? { properties_count: properties } : {}),
    ...(doors !== undefined ? { doors } : {}),
    ...(rent !== null ? { monthly_rent_cents: rent } : {}),
    ...(fee_bps !== undefined ? { fee_bps } : {}),
  };
}

function LeadFields({
  form,
  set,
  feeHint,
}: {
  form: LeadForm;
  set: (patch: Partial<LeadForm>) => void;
  feeHint: string;
}) {
  const input = (
    key: keyof LeadForm,
    label: string,
    props?: React.ComponentProps<"input">
  ) => (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <Input
        value={form[key]}
        onChange={(e) => set({ [key]: e.target.value })}
        {...props}
      />
    </div>
  );
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        {input("name", "Name", { required: true })}
        {input("company", "Company")}
      </div>
      <div className="grid grid-cols-2 gap-3">
        {input("email", "Email", { type: "email" })}
        {input("phone", "Phone")}
      </div>
      {input("address", "Address", {
        placeholder: "Where their properties are",
      })}
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {input("properties", "Properties", { inputMode: "numeric" })}
        {input("doors", "Doors", { inputMode: "numeric" })}
        {input("rent", "Monthly rent ($)", {
          inputMode: "decimal",
          placeholder: "All doors",
        })}
        {input("fee", "Fee (%)", {
          inputMode: "decimal",
          placeholder: feeHint,
        })}
      </div>
      <div className="space-y-1.5">
        <Label>Source</Label>
        <select
          value={form.source}
          onChange={(e) => set({ source: e.target.value })}
          className={selectCls}
        >
          {LEAD_SOURCES.map((s) => (
            <option key={s} value={s}>
              {humanize(s)}
            </option>
          ))}
        </select>
      </div>
      <div className="space-y-1.5">
        <Label>Notes</Label>
        <textarea
          value={form.notes}
          onChange={(e) => set({ notes: e.target.value })}
          rows={3}
          className={field}
        />
      </div>
    </div>
  );
}

function NewLeadDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (l: OwnerLead) => void;
}) {
  const [form, setForm] = useState<LeadForm>(EMPTY_LEAD);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const body = formToInput(form);
    if (typeof body === "string") {
      toast.error(body);
      return;
    }
    setBusy(true);
    try {
      const l = await crm.createLead(body);
      toast.success(`Added ${l.name} to the pipeline`);
      setForm(EMPTY_LEAD);
      onCreated(l);
    } catch (err) {
      toast.error(errMsg(err, "Couldn't add the lead"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-2xl overflow-y-auto">
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>New lead</DialogTitle>
            <DialogDescription>
              An owner who might hire you. Leave the fee blank to use your
              standard management fee.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5">
            <LeadFields
              form={form}
              set={(p) => setForm((f) => ({ ...f, ...p }))}
              feeHint="Standard"
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !form.name.trim()}>
              {busy ? "Adding…" : "Add lead"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function LeadDetails({
  lead,
  manage,
  onSaved,
  onMove,
  onConverted,
}: {
  lead: OwnerLead;
  manage: boolean;
  onSaved: () => void;
  onMove: (s: LeadStatus) => void;
  onConverted: () => void;
}) {
  const initial = leadToForm(lead);
  const [form, setForm] = useState<LeadForm>(initial);
  const [busy, setBusy] = useState(false);
  const [converting, setConverting] = useState(false);
  const [printing, setPrinting] = useState(false);

  const dirty = (Object.keys(initial) as (keyof LeadForm)[]).some(
    (k) => form[k] !== initial[k]
  );

  async function save(e: React.FormEvent) {
    e.preventDefault();
    const body = formToInput(form, initial.fee);
    if (typeof body === "string") {
      toast.error(body);
      return;
    }
    setBusy(true);
    try {
      await crm.updateLead(lead.id, body);
      toast.success("Lead saved");
      onSaved();
    } catch (err) {
      toast.error(errMsg(err, "Couldn't save the lead"));
    } finally {
      setBusy(false);
    }
  }

  async function print() {
    setPrinting(true);
    try {
      await openPdf(crm.proposalPath(lead.id));
    } catch (err) {
      toast.error(errMsg(err, "Couldn't open the proposal"));
    } finally {
      setPrinting(false);
    }
  }

  async function convert() {
    if (
      !window.confirm(
        `Make ${lead.company || lead.name} an owner? This marks the lead won and moves its timeline to the new owner.`
      )
    )
      return;
    setConverting(true);
    try {
      await crm.convertLead(lead.id);
      toast.success(`${lead.company || lead.name} is now an owner`);
      onConverted();
    } catch (err) {
      toast.error(errMsg(err, "Couldn't convert the lead"));
    } finally {
      setConverting(false);
    }
  }

  return (
    <section className="space-y-4">
      <div className="grid grid-cols-3 gap-3">
        <Stat
          label="Monthly fee"
          value={money(lead.monthly_fee_cents)}
          sub={`${pct(lead.fee_bps)} of ${money(lead.monthly_rent_cents)}`}
        />
        <Stat
          label="Doors"
          value={String(lead.doors)}
          sub={`${lead.properties_count} properties`}
        />
        <Stat
          label="Assigned"
          value={lead.assigned_name ?? "—"}
          sub={`Added ${new Date(lead.created_at).toLocaleDateString()}`}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {manage ? (
          <label className="flex items-center gap-2 text-sm font-semibold text-ink-3">
            Stage
            <select
              value={lead.status}
              onChange={(e) => onMove(e.target.value as LeadStatus)}
              className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm font-normal text-ink"
            >
              {LEAD_STATUSES.map((s) => (
                <option key={s} value={s}>
                  {STAGE_LABEL[s]}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <Badge tone={stageTone(lead.status)}>
            {STAGE_LABEL[lead.status]}
          </Badge>
        )}
        <div className="ml-auto flex flex-wrap gap-2">
          <Button variant="outline" onClick={print} disabled={printing}>
            <Printer size={16} aria-hidden />
            {printing ? "Opening…" : "Print proposal"}
          </Button>
          {manage && !lead.owner_id && (
            <Button onClick={convert} disabled={converting}>
              <UserPlus size={16} aria-hidden />
              {converting ? "Converting…" : "Convert to owner"}
            </Button>
          )}
          {lead.owner_id && <Badge tone="good">Converted to an owner</Badge>}
        </div>
      </div>
      {lead.status === "lost" && lead.lost_reason && (
        <p className="rounded-xl bg-bad-soft px-3 py-2 text-sm text-bad">
          Lost: {lead.lost_reason}
        </p>
      )}

      <form onSubmit={save} className="space-y-3">
        <fieldset disabled={!manage}>
          <LeadFields
            form={form}
            set={(p) => setForm((f) => ({ ...f, ...p }))}
            feeHint="Standard"
          />
        </fieldset>
        {manage && (
          <div className="flex justify-end">
            <Button
              type="submit"
              disabled={busy || !dirty || !form.name.trim()}
            >
              {busy ? "Saving…" : "Save changes"}
            </Button>
          </div>
        )}
      </form>
    </section>
  );
}
