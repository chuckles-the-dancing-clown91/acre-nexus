"use client";

// The owner-lead pipeline: owners who might hire you, from first contact to
// a signed management agreement. Stage totals and win rates, a board to move
// leads along, and the side-panel details with the proposal PDF and
// "convert to owner".

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  ChevronLeft,
  ChevronRight,
  Plus,
  Printer,
  UserPlus,
} from "lucide-react";
import { toast } from "sonner";
import {
  crm,
  LEAD_SOURCES,
  LEAD_STATUSES,
  money,
  openPdf,
  pct,
  toCents,
  type LeadStatus,
  type OwnerLead,
  type OwnerLeadInput,
  type PipelineSummary,
} from "@/lib/backoffice";
import type { Member } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import {
  errMsg,
  humanize,
  MiniStat,
  STAGE_LABEL,
  stageTone,
  type Opened,
} from "./shared";
import { followUpState } from "./timeline";

export type MoveLead = (l: OwnerLead, s: LeadStatus) => void;

/** Moving a lead between stages; "lost" asks why first. */
export function useLeadMover(): { move: MoveLead; dialog: React.ReactNode } {
  const qc = useQueryClient();
  const [lostFor, setLostFor] = useState<OwnerLead | null>(null);

  async function commit(l: OwnerLead, status: LeadStatus, reason?: string) {
    try {
      await crm.updateLead(l.id, {
        status,
        ...(status === "lost" ? { lost_reason: reason ?? "" } : {}),
      });
      toast.success(`${l.name} moved to ${STAGE_LABEL[status]}`);
      await qc.invalidateQueries({ queryKey: ["crm"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't move the lead"));
    }
  }

  const move: MoveLead = (l, status) => {
    if (status === l.status) return;
    if (status === "lost") {
      setLostFor(l);
      return;
    }
    void commit(l, status);
  };

  const dialog = lostFor && (
    <LostReasonDialog
      key={lostFor.id}
      lead={lostFor}
      onClose={() => setLostFor(null)}
      onConfirm={(reason) => {
        const l = lostFor;
        setLostFor(null);
        void commit(l, "lost", reason);
      }}
    />
  );

  return { move, dialog };
}

export function PipelineTab({
  leads,
  summary,
  loading,
  error,
  manage,
  onMove,
  onOpen,
}: {
  leads: OwnerLead[] | undefined;
  summary: PipelineSummary | undefined;
  loading: boolean;
  error: Error | null;
  manage: boolean;
  onMove: MoveLead;
  onOpen: (o: Opened) => void;
}) {
  const [creating, setCreating] = useState(false);

  return (
    <div className="space-y-5">
      {summary && <SummaryStrip summary={summary} />}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-[13px] text-fg-3">
          Owners who might hire you to manage their properties.
        </p>
        {manage && (
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus />
            New lead
          </Button>
        )}
      </div>

      {loading && <Skeleton className="h-72 rounded-2xl" />}
      {error && (
        <p className="text-[13px] text-bad">
          Couldn&apos;t load leads: {error.message}
        </p>
      )}
      {leads && (
        <div className="-mx-4 overflow-x-auto px-4 pb-2 sm:mx-0 sm:px-0">
          <div className="grid min-w-[1000px] grid-cols-5 gap-3">
            {LEAD_STATUSES.map((stage) => {
              const cards = leads.filter((l) => l.status === stage);
              return (
                <div
                  key={stage}
                  className="flex flex-col gap-2 rounded-2xl border border-line bg-fill/50 p-2.5"
                >
                  <div className="flex items-center justify-between px-1">
                    <Badge tone={stageTone(stage)}>{STAGE_LABEL[stage]}</Badge>
                    <span className="figure text-xs text-fg-3">
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
                    <div className="rounded-xl border border-dashed border-line px-3 py-6 text-center text-xs text-fg-4">
                      No leads here
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {creating && (
        <NewLeadDialog
          onClose={() => setCreating(false)}
          onCreated={(l) => {
            setCreating(false);
            onOpen({ type: "owner_lead", id: l.id, name: l.name });
          }}
        />
      )}
    </div>
  );
}

function SummaryStrip({ summary }: { summary: PipelineSummary }) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5">
        {LEAD_STATUSES.map((s) => {
          const st = summary.stages.find((x) => x.status === s);
          return (
            <Stat
              key={s}
              label={STAGE_LABEL[s]}
              value={st?.leads ?? 0}
              hint={`${st?.doors ?? 0} doors · ${money(st?.monthly_fee_cents ?? 0)}/mo`}
            />
          );
        })}
      </div>
      <Panel className="flex flex-wrap items-center gap-x-8 gap-y-3 px-5 py-4">
        <div>
          <div className="eyebrow">Weighted monthly fees</div>
          <div className="figure mt-1 text-[22px] font-semibold text-fg">
            {money(summary.weighted_monthly_fee_cents)}
          </div>
        </div>
        <div>
          <div className="eyebrow">Win rate</div>
          <div className="figure mt-1 text-[22px] font-semibold text-fg">
            {pct(summary.win_bps)}
          </div>
        </div>
        {summary.follow_ups_due > 0 && (
          <Badge tone="warn">{summary.follow_ups_due} follow-ups due</Badge>
        )}
        {summary.by_source.length > 0 && (
          <div className="flex flex-1 flex-wrap items-center gap-1.5 lg:justify-end">
            <span className="text-xs text-fg-3">Win rate by source</span>
            {summary.by_source.map((s) => (
              <Badge key={s.source}>
                {humanize(s.source)} · {pct(s.win_bps)}
                <span className="text-fg-4">
                  ({s.won}/{s.leads})
                </span>
              </Badge>
            ))}
          </div>
        )}
      </Panel>
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
    <div className="rounded-xl border border-line bg-surface">
      <button
        type="button"
        onClick={onOpen}
        className="w-full space-y-1.5 rounded-t-xl p-3 text-left transition hover:bg-fill-2"
      >
        <div className="text-[14px] leading-tight font-medium text-fg">
          {l.name}
        </div>
        {l.company && <div className="text-xs text-fg-3">{l.company}</div>}
        <div className="text-xs text-fg-2">
          {l.doors} doors · {l.properties_count}{" "}
          {l.properties_count === 1 ? "property" : "properties"}
        </div>
        <div className="text-[13px]">
          <span className="figure font-medium text-fg">
            {money(l.monthly_fee_cents)}
          </span>
          <span className="text-fg-3">/mo · {pct(l.fee_bps)}</span>
        </div>
        <div className="text-xs text-fg-3">
          {humanize(l.source)}
          {l.assigned_name && ` · ${l.assigned_name}`}
        </div>
        {fu && <Badge tone={fu.tone}>Follow up · {fu.label}</Badge>}
        {l.status === "lost" && l.lost_reason && (
          <div className="line-clamp-2 text-xs text-fg-3">
            Lost: {l.lost_reason}
          </div>
        )}
        {l.owner_id && <Badge tone="good">Now an owner</Badge>}
      </button>
      {manage && (
        <div className="flex items-center gap-1 border-t border-line px-1.5 py-1.5">
          <Button
            size="icon"
            variant="ghost"
            className="size-7"
            disabled={idx <= 0}
            onClick={() => onMove(LEAD_STATUSES[idx - 1])}
            aria-label="Move back a stage"
            title="Move back a stage"
          >
            <ChevronLeft />
          </Button>
          <select
            value={l.status}
            onChange={(e) => onMove(e.target.value as LeadStatus)}
            className={cn(fieldClass, "min-w-0 flex-1 px-1.5 py-1 text-xs")}
            aria-label={`Stage for ${l.name}`}
          >
            {LEAD_STATUSES.map((s) => (
              <option key={s} value={s}>
                {STAGE_LABEL[s]}
              </option>
            ))}
          </select>
          <Button
            size="icon"
            variant="ghost"
            className="size-7"
            disabled={idx >= LEAD_STATUSES.length - 1}
            onClick={() => onMove(LEAD_STATUSES[idx + 1])}
            aria-label="Move forward a stage"
            title="Move forward a stage"
          >
            <ChevronRight />
          </Button>
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
  lead: OwnerLead;
  onClose: () => void;
  onConfirm: (reason: string) => void;
}) {
  const [reason, setReason] = useState("");
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Mark as lost
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Why didn&apos;t {lead.name} sign? It helps spot patterns later.
        </DialogDescription>
        <form
          className="mt-4 space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            onConfirm(reason.trim());
          }}
        >
          <Field label="Reason">
            {(f) => (
              <textarea
                {...f}
                autoFocus
                rows={3}
                className={cn(fieldClass, "w-full")}
                placeholder="e.g. Went with another manager, fee too high, decided to sell"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
              />
            )}
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="danger">
              Mark as lost
            </Button>
          </div>
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
  assigned: string;
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
  assigned: "",
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
    assigned: l.assigned_to ?? "",
  };
}

/** Form to API body, or an error string when something doesn't parse. */
function formToInput(f: LeadForm, initial?: LeadForm): OwnerLeadInput | string {
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
  if (f.fee.trim() !== "" && f.fee.trim() !== initial?.fee) {
    const n = Number(f.fee.replace("%", ""));
    if (!Number.isFinite(n) || n < 0 || n > 50)
      return "Fee should be a percent between 0 and 50.";
    fee_bps = Math.round(n * 100);
  }
  // Only send an assignee when it changed; the server can't clear one.
  const assignedChanged = f.assigned !== (initial?.assigned ?? "");
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
    ...(assignedChanged && f.assigned ? { assigned_to: f.assigned } : {}),
  };
}

function LeadFields({
  form,
  set,
  members,
}: {
  form: LeadForm;
  set: (patch: Partial<LeadForm>) => void;
  members: Member[];
}) {
  const input = (
    key: keyof LeadForm,
    label: string,
    props?: React.ComponentProps<"input">
  ) => (
    <Field label={label}>
      {(f) => (
        <Input
          {...f}
          {...props}
          value={form[key]}
          onChange={(e) => set({ [key]: e.target.value })}
        />
      )}
    </Field>
  );
  return (
    <div className="space-y-3">
      <div className="grid gap-3 sm:grid-cols-2">
        {input("name", "Name", { required: true })}
        {input("company", "Company")}
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        {input("email", "Email", { type: "email" })}
        {input("phone", "Phone", { type: "tel" })}
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
          placeholder: "Standard",
        })}
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Source">
          {(f) => (
            <select
              {...f}
              className={cn(fieldClass, "h-11 w-full")}
              value={form.source}
              onChange={(e) => set({ source: e.target.value })}
            >
              {LEAD_SOURCES.map((s) => (
                <option key={s} value={s}>
                  {humanize(s)}
                </option>
              ))}
            </select>
          )}
        </Field>
        {members.length > 0 && (
          <Field label="Assigned to">
            {(f) => (
              <select
                {...f}
                className={cn(fieldClass, "h-11 w-full")}
                value={form.assigned}
                onChange={(e) => set({ assigned: e.target.value })}
              >
                <option value="" disabled>
                  Nobody yet
                </option>
                {members.map((m) => (
                  <option key={m.user_id} value={m.user_id}>
                    {m.name}
                  </option>
                ))}
              </select>
            )}
          </Field>
        )}
      </div>
      <Field label="Notes">
        {(f) => (
          <textarea
            {...f}
            rows={3}
            className={cn(fieldClass, "w-full")}
            value={form.notes}
            onChange={(e) => set({ notes: e.target.value })}
          />
        )}
      </Field>
    </div>
  );
}

function NewLeadDialog({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (l: OwnerLead) => void;
}) {
  const qc = useQueryClient();
  const [form, setForm] = useState<LeadForm>(EMPTY_LEAD);
  const [busy, setBusy] = useState(false);

  async function submit() {
    const body = formToInput(form);
    if (typeof body === "string") {
      toast.error(body);
      return;
    }
    setBusy(true);
    try {
      const l = await crm.createLead(body);
      toast.success(`Added ${l.name} to the pipeline`);
      await qc.invalidateQueries({ queryKey: ["crm"] });
      onCreated(l);
    } catch (e) {
      toast.error(errMsg(e, "Couldn't add the lead"));
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-2xl overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          New lead
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          An owner who might hire you. Leave the fee blank to use your standard
          management fee.
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <LeadFields
            form={form}
            set={(p) => setForm((f) => ({ ...f, ...p }))}
            members={[]}
          />
          <div className="mt-5 flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={busy} disabled={!form.name.trim()}>
              {!busy && <Plus />}
              Add lead
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function LeadDetails({
  lead,
  manage,
  members,
  onMove,
  onConverted,
}: {
  lead: OwnerLead;
  manage: boolean;
  members: Member[];
  onMove: (s: LeadStatus) => void;
  onConverted: () => void;
}) {
  const qc = useQueryClient();
  const initial = leadToForm(lead);
  const [form, setForm] = useState<LeadForm>(initial);
  const [busy, setBusy] = useState(false);
  const [converting, setConverting] = useState(false);
  const [printing, setPrinting] = useState(false);

  const dirty = (Object.keys(initial) as (keyof LeadForm)[]).some(
    (k) => form[k] !== initial[k]
  );

  async function save() {
    const body = formToInput(form, initial);
    if (typeof body === "string") {
      toast.error(body);
      return;
    }
    setBusy(true);
    try {
      await crm.updateLead(lead.id, body);
      toast.success("Lead saved");
      await qc.invalidateQueries({ queryKey: ["crm"] });
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the lead"));
    } finally {
      setBusy(false);
    }
  }

  async function print() {
    setPrinting(true);
    try {
      await openPdf(crm.proposalPath(lead.id));
    } catch (e) {
      toast.error(errMsg(e, "Couldn't open the proposal"));
    } finally {
      setPrinting(false);
    }
  }

  async function convert() {
    const who = lead.company || lead.name;
    if (
      !window.confirm(
        `Make ${who} an owner? This marks the lead won and moves its timeline to the new owner.`
      )
    )
      return;
    setConverting(true);
    try {
      await crm.convertLead(lead.id);
      toast.success(`${who} is now an owner`);
      await qc.invalidateQueries({ queryKey: ["crm"] });
      onConverted();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't convert the lead"));
    } finally {
      setConverting(false);
    }
  }

  return (
    <section className="space-y-5">
      <div className="grid grid-cols-3 gap-2">
        <MiniStat
          label="Monthly fee"
          value={money(lead.monthly_fee_cents)}
          sub={`${pct(lead.fee_bps)} of ${money(lead.monthly_rent_cents)}`}
        />
        <MiniStat
          label="Doors"
          value={String(lead.doors)}
          sub={`${lead.properties_count} properties`}
        />
        <MiniStat
          label="Assigned"
          value={lead.assigned_name ?? "Nobody"}
          sub={`Added ${new Date(lead.created_at).toLocaleDateString()}`}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {manage ? (
          <label className="flex items-center gap-2 text-[13px] text-fg-3">
            Stage
            <select
              value={lead.status}
              onChange={(e) => onMove(e.target.value as LeadStatus)}
              className={fieldClass}
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
          <Button
            size="sm"
            variant="secondary"
            loading={printing}
            onClick={print}
          >
            {!printing && <Printer />}
            Print proposal
          </Button>
          {manage && !lead.owner_id && (
            <Button size="sm" loading={converting} onClick={convert}>
              {!converting && <UserPlus />}
              Convert to owner
            </Button>
          )}
          {lead.owner_id && <Badge tone="good">Converted to an owner</Badge>}
        </div>
      </div>
      {lead.status === "lost" && lead.lost_reason && (
        <p className="rounded-xl border border-bad/25 bg-bad/10 px-3 py-2 text-[13px] text-bad">
          Lost: {lead.lost_reason}
        </p>
      )}

      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <fieldset disabled={!manage}>
          <LeadFields
            form={form}
            set={(p) => setForm((f) => ({ ...f, ...p }))}
            members={members}
          />
        </fieldset>
        {manage && (
          <div className="mt-3 flex justify-end">
            <Button
              type="submit"
              loading={busy}
              disabled={!dirty || !form.name.trim()}
            >
              Save changes
            </Button>
          </div>
        )}
      </form>
    </section>
  );
}
