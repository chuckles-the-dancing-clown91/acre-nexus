"use client";

// Insurance on a property: who carries it, what it covers, what it costs,
// and when it renews.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pencil, Phone, Plus, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import {
  centsFrom,
  day,
  daysUntil,
  dollarsField,
  label,
  POLICY_KINDS,
  records,
  type Policy,
  type PolicyInput,
} from "@/lib/propertyRecords";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { F, Fact, FormDialog, input, why } from "./bits";

type Draft = Record<
  | "carrier"
  | "kind"
  | "status"
  | "policy_number"
  | "effective_on"
  | "expires_on"
  | "premium"
  | "coverage"
  | "deductible"
  | "agent_name"
  | "agent_phone"
  | "agent_email"
  | "notes",
  string
>;

function draftOf(p?: Policy): Draft {
  return {
    carrier: p?.carrier ?? "",
    kind: p?.kind ?? "property",
    status: p?.status ?? "active",
    policy_number: p?.policy_number ?? "",
    effective_on: p?.effective_on ?? "",
    expires_on: p?.expires_on ?? "",
    premium: dollarsField(p?.premium_cents),
    coverage: dollarsField(p?.coverage_cents),
    deductible: dollarsField(p?.deductible_cents),
    agent_name: p?.agent_name ?? "",
    agent_phone: p?.agent_phone ?? "",
    agent_email: p?.agent_email ?? "",
    notes: p?.notes ?? "",
  };
}

function toInput(d: Draft, keep: string[]): PolicyInput {
  const { premium, coverage, deductible, ...rest } = d;
  return {
    ...rest,
    premium_cents: centsFrom(premium),
    coverage_cents: centsFrom(coverage),
    deductible_cents: centsFrom(deductible),
    document_ids: keep,
  };
}

export function Insurance({
  propertyId,
  manage,
  floodZone,
}: {
  propertyId: string;
  manage: boolean;
  floodZone?: string | null;
}) {
  const qc = useQueryClient();
  const policies = useQuery({
    queryKey: ["policies", propertyId],
    queryFn: () => records.policies(propertyId),
  });
  const [editing, setEditing] = useState<Policy | "new" | null>(null);
  const [d, setD] = useState<Draft>(draftOf());
  const [busy, setBusy] = useState(false);
  const set = (k: keyof Draft) => (e: { target: { value: string } }) =>
    setD({ ...d, [k]: e.target.value });
  const open = (p: Policy | "new") => {
    setD(draftOf(p === "new" ? undefined : p));
    setEditing(p);
  };
  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["policies", propertyId] });
    void qc.invalidateQueries({ queryKey: ["attention", propertyId] });
  };

  async function save() {
    setBusy(true);
    try {
      if (editing === "new")
        await records.createPolicy(propertyId, toInput(d, []));
      else if (editing)
        await records.updatePolicy(
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
    if (!editing || editing === "new" || !confirm("Delete this policy?"))
      return;
    setBusy(true);
    try {
      await records.deletePolicy(propertyId, editing.id);
      setEditing(null);
      refresh();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const rows = policies.data ?? [];
  const annual = rows
    .filter((p) => p.status === "active")
    .reduce((s, p) => s + (p.premium_cents ?? 0), 0);

  return (
    <Panel>
      <PanelHeader
        title="Insurance"
        description={
          annual
            ? `$${Math.round(annual / 100).toLocaleString()} a year in premiums`
            : "Policies on this property."
        }
        action={
          manage && (
            <Button size="sm" variant="secondary" onClick={() => open("new")}>
              <Plus />
              Policy
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5 pt-4">
        {floodZone && (
          <p className="text-xs text-fg-3">
            FEMA flood zone{" "}
            <span className="font-medium text-fg">{floodZone}</span>
          </p>
        )}
        {policies.isLoading && <Skeleton className="h-24" />}
        {policies.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<ShieldCheck />}
            title="No policies on file"
            description="Add the hazard policy and any flood, liability or umbrella cover so renewals don't sneak up."
            className="py-6"
          />
        )}
        <div className="grid gap-3 lg:grid-cols-2">
          {rows.map((p) => {
            const left = p.expires_on ? daysUntil(p.expires_on) : null;
            return (
              <div
                key={p.id}
                className={cn(
                  "rounded-xl border border-line p-4",
                  p.status !== "active" && "opacity-60"
                )}
              >
                <div className="flex items-start gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="text-[14px] font-semibold text-fg">
                      {p.carrier}
                    </div>
                    <div className="mt-0.5 flex flex-wrap gap-1.5">
                      <Badge>{label(p.kind)}</Badge>
                      {p.status !== "active" && (
                        <Badge tone="bad">{p.status}</Badge>
                      )}
                      {p.status === "active" && left != null && left < 0 && (
                        <Badge tone="bad">lapsed</Badge>
                      )}
                      {p.status === "active" &&
                        left != null &&
                        left >= 0 &&
                        left <= 45 && <Badge tone="warn">renews soon</Badge>}
                    </div>
                  </div>
                  {manage && (
                    <button
                      type="button"
                      aria-label={`Edit ${p.carrier} policy`}
                      onClick={() => open(p)}
                      className="rounded-lg p-1.5 text-fg-3 hover:bg-fill-2 hover:text-fg"
                    >
                      <Pencil className="size-4" />
                    </button>
                  )}
                </div>
                <dl className="mt-2 divide-y divide-line">
                  <Fact label="Policy">{p.policy_number}</Fact>
                  <Fact label="Coverage">{p.coverage_label}</Fact>
                  <Fact label="Deductible">{p.deductible_label}</Fact>
                  <Fact label="Premium">
                    {p.premium_label && `${p.premium_label} / yr`}
                  </Fact>
                  <Fact label="Term">
                    {(p.effective_on || p.expires_on) &&
                      `${day(p.effective_on) || "?"} to ${day(p.expires_on) || "?"}`}
                  </Fact>
                  <Fact label="Agent">
                    {p.agent_name && (
                      <span className="inline-flex items-center gap-1.5">
                        {p.agent_name}
                        {p.agent_phone && (
                          <a
                            href={`tel:${p.agent_phone}`}
                            className="text-accent"
                            aria-label={`Call ${p.agent_name}`}
                          >
                            <Phone className="size-3.5" />
                          </a>
                        )}
                      </span>
                    )}
                  </Fact>
                </dl>
              </div>
            );
          })}
        </div>
      </div>

      <FormDialog
        open={!!editing}
        onOpenChange={(o) => !o && setEditing(null)}
        title={editing === "new" ? "Add a policy" : "Edit policy"}
        busy={busy}
        onSave={save}
        onDelete={editing && editing !== "new" ? remove : undefined}
        wide
      >
        <F label="Carrier">
          <input
            className={input}
            required
            value={d.carrier}
            onChange={set("carrier")}
          />
        </F>
        <F label="Kind">
          <select className={input} value={d.kind} onChange={set("kind")}>
            {POLICY_KINDS.map((k) => (
              <option key={k} value={k}>
                {label(k)}
              </option>
            ))}
          </select>
        </F>
        <F label="Policy number">
          <input
            className={input}
            value={d.policy_number}
            onChange={set("policy_number")}
          />
        </F>
        <F label="Status">
          <select className={input} value={d.status} onChange={set("status")}>
            {["active", "cancelled", "expired"].map((k) => (
              <option key={k} value={k}>
                {label(k)}
              </option>
            ))}
          </select>
        </F>
        <F label="Starts">
          <input
            type="date"
            className={input}
            value={d.effective_on}
            onChange={set("effective_on")}
          />
        </F>
        <F label="Renews">
          <input
            type="date"
            className={input}
            value={d.expires_on}
            onChange={set("expires_on")}
          />
        </F>
        <F label="Coverage ($)">
          <input
            className={input}
            inputMode="decimal"
            value={d.coverage}
            onChange={set("coverage")}
          />
        </F>
        <F label="Deductible ($)">
          <input
            className={input}
            inputMode="decimal"
            value={d.deductible}
            onChange={set("deductible")}
          />
        </F>
        <F label="Annual premium ($)">
          <input
            className={input}
            inputMode="decimal"
            value={d.premium}
            onChange={set("premium")}
          />
        </F>
        <F label="Agent">
          <input
            className={input}
            value={d.agent_name}
            onChange={set("agent_name")}
          />
        </F>
        <F label="Agent phone">
          <input
            className={input}
            type="tel"
            value={d.agent_phone}
            onChange={set("agent_phone")}
          />
        </F>
        <F label="Agent email">
          <input
            className={input}
            type="email"
            value={d.agent_email}
            onChange={set("agent_email")}
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
