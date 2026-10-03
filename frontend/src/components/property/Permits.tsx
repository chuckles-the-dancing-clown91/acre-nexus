"use client";

// Permits pulled on a property: what each covers, where it stands with the
// building department, and the dates that matter.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FileBadge, Pencil, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  centsFrom,
  day,
  daysUntil,
  dollarsField,
  label,
  PERMIT_KINDS,
  PERMIT_STATUSES,
  records,
  type Permit,
  type PermitInput,
} from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, FormDialog, input, why } from "./bits";

type Draft = Record<
  | "description"
  | "kind"
  | "status"
  | "permit_number"
  | "jurisdiction"
  | "applied_on"
  | "issued_on"
  | "expires_on"
  | "inspection_on"
  | "finaled_on"
  | "contractor_name"
  | "valuation"
  | "fee"
  | "notes",
  string
>;

function draftOf(p?: Permit): Draft {
  return {
    description: p?.description ?? "",
    kind: p?.kind ?? "building",
    status: p?.status ?? "applied",
    permit_number: p?.permit_number ?? "",
    jurisdiction: p?.jurisdiction ?? "",
    applied_on: p?.applied_on ?? "",
    issued_on: p?.issued_on ?? "",
    expires_on: p?.expires_on ?? "",
    inspection_on: p?.inspection_on ?? "",
    finaled_on: p?.finaled_on ?? "",
    contractor_name: p?.contractor_name ?? "",
    valuation: dollarsField(p?.valuation_cents),
    fee: dollarsField(p?.fee_cents),
    notes: p?.notes ?? "",
  };
}

function toInput(d: Draft, keep: string[]): PermitInput {
  const { valuation, fee, ...rest } = d;
  return {
    ...rest,
    valuation_cents: centsFrom(valuation),
    fee_cents: centsFrom(fee),
    document_ids: keep,
  };
}

const tone = (s: string) =>
  s === "finaled"
    ? "good"
    : s === "expired" || s === "void"
      ? "bad"
      : s === "inspection"
        ? "info"
        : "warn";

export function Permits({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const permits = useQuery({
    queryKey: ["permits", propertyId],
    queryFn: () => records.permits(propertyId),
  });
  const [editing, setEditing] = useState<Permit | "new" | null>(null);
  const [d, setD] = useState<Draft>(draftOf());
  const [busy, setBusy] = useState(false);

  const open = (p: Permit | "new") => {
    setD(draftOf(p === "new" ? undefined : p));
    setEditing(p);
  };
  const set = (k: keyof Draft) => (e: { target: { value: string } }) =>
    setD({ ...d, [k]: e.target.value });

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["permits", propertyId] });
    void qc.invalidateQueries({ queryKey: ["attention", propertyId] });
  };

  async function save() {
    setBusy(true);
    try {
      if (editing === "new")
        await records.createPermit(propertyId, toInput(d, []));
      else if (editing)
        await records.updatePermit(
          propertyId,
          editing.id,
          toInput(d, editing.document_ids)
        );
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!editing || editing === "new") return;
    if (!confirm("Delete this permit?")) return;
    setBusy(true);
    try {
      await records.deletePermit(propertyId, editing.id);
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const rows = permits.data ?? [];

  return (
    <Panel>
      <PanelHeader
        title="Permits"
        description="Open permits first. A lapsing permit or an inspection coming up lands on the to-do list."
        action={
          manage && (
            <Button size="sm" variant="secondary" onClick={() => open("new")}>
              <Plus />
              Permit
            </Button>
          )
        }
      />
      <div className="p-2 pt-3">
        {permits.isLoading && <Skeleton className="m-3 h-20" />}
        {permits.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<FileBadge />}
            title="No permits on file"
            description="Record permits for remodels, roofs, panels, decks and fences so the history stays with the property."
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {rows.map((p) => (
            <li key={p.id} className="flex items-start gap-3 px-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-[14px] font-medium text-fg">
                    {p.description}
                  </span>
                  <Badge tone={tone(p.status)}>{label(p.status)}</Badge>
                  <Badge>{label(p.kind)}</Badge>
                </div>
                <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-fg-3">
                  {p.permit_number && <span>#{p.permit_number}</span>}
                  {p.jurisdiction && <span>{p.jurisdiction}</span>}
                  {p.contractor_name && <span>{p.contractor_name}</span>}
                  {p.issued_on && <span>Issued {day(p.issued_on)}</span>}
                  {p.finaled_on && <span>Finaled {day(p.finaled_on)}</span>}
                  {p.open && p.inspection_on && (
                    <span className="text-info">
                      Inspection {day(p.inspection_on)}
                    </span>
                  )}
                  {p.open && p.expires_on && (
                    <span className={dueTone(p.expires_on)}>
                      {daysUntil(p.expires_on) < 0 ? "Expired" : "Expires"}{" "}
                      {day(p.expires_on)}
                    </span>
                  )}
                  {p.fee_label && <span>Fee {p.fee_label}</span>}
                  {p.valuation_label && <span>Job {p.valuation_label}</span>}
                </div>
                {p.notes && (
                  <div className="mt-1 text-xs text-fg-3">{p.notes}</div>
                )}
              </div>
              {manage && (
                <button
                  type="button"
                  aria-label={`Edit ${p.description}`}
                  onClick={() => open(p)}
                  className="rounded-lg p-1.5 text-fg-3 hover:bg-fill-2 hover:text-fg"
                >
                  <Pencil className="size-4" />
                </button>
              )}
            </li>
          ))}
        </ul>
      </div>

      <FormDialog
        open={!!editing}
        onOpenChange={(o) => !o && setEditing(null)}
        title={editing === "new" ? "Add a permit" : "Edit permit"}
        busy={busy}
        onSave={save}
        onDelete={editing && editing !== "new" ? remove : undefined}
        wide
      >
        <F label="What it covers" className="sm:col-span-2">
          <input
            className={input}
            required
            placeholder="Rebuild rear deck"
            value={d.description}
            onChange={set("description")}
          />
        </F>
        <F label="Kind">
          <select className={input} value={d.kind} onChange={set("kind")}>
            {PERMIT_KINDS.map((k) => (
              <option key={k} value={k}>
                {label(k)}
              </option>
            ))}
          </select>
        </F>
        <F label="Status">
          <select className={input} value={d.status} onChange={set("status")}>
            {PERMIT_STATUSES.map((k) => (
              <option key={k} value={k}>
                {label(k)}
              </option>
            ))}
          </select>
        </F>
        <F label="Permit number">
          <input
            className={input}
            value={d.permit_number}
            onChange={set("permit_number")}
          />
        </F>
        <F label="Issued by">
          <input
            className={input}
            placeholder="City of Portland"
            value={d.jurisdiction}
            onChange={set("jurisdiction")}
          />
        </F>
        <F label="Applied">
          <input
            type="date"
            className={input}
            value={d.applied_on}
            onChange={set("applied_on")}
          />
        </F>
        <F label="Issued">
          <input
            type="date"
            className={input}
            value={d.issued_on}
            onChange={set("issued_on")}
          />
        </F>
        <F label="Next inspection">
          <input
            type="date"
            className={input}
            value={d.inspection_on}
            onChange={set("inspection_on")}
          />
        </F>
        <F label="Expires">
          <input
            type="date"
            className={input}
            value={d.expires_on}
            onChange={set("expires_on")}
          />
        </F>
        <F label="Finaled">
          <input
            type="date"
            className={input}
            value={d.finaled_on}
            onChange={set("finaled_on")}
          />
        </F>
        <F label="Contractor">
          <input
            className={input}
            value={d.contractor_name}
            onChange={set("contractor_name")}
          />
        </F>
        <F label="Job value ($)">
          <input
            className={input}
            inputMode="decimal"
            value={d.valuation}
            onChange={set("valuation")}
          />
        </F>
        <F label="Permit fee ($)">
          <input
            className={input}
            inputMode="decimal"
            value={d.fee}
            onChange={set("fee")}
          />
        </F>
        <F label="Notes" className="sm:col-span-2">
          <textarea
            className={`${input} min-h-[64px]`}
            value={d.notes}
            onChange={set("notes")}
          />
        </F>
      </FormDialog>
    </Panel>
  );
}

function dueTone(iso: string): string {
  const n = daysUntil(iso);
  return n < 0 ? "font-medium text-bad" : n <= 30 ? "text-warn" : "";
}
