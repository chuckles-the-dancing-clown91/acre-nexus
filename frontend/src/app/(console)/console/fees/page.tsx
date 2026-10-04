"use client";

// The fee schedule: fees, discounts, rebates and amenities. Conditional items
// land on a lease by themselves when the resident qualifies (a pet, military
// service, a vehicle). Reading needs fee:read; changes need fee:manage.

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus, Receipt, Trash2 } from "lucide-react";
import { api, type Fee } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import {
  F,
  humanize,
  inputClass,
  NoAccess,
  Th,
  toCents,
  useRun,
} from "../leases/_ui/shared";

const KINDS = ["fee", "discount", "rebate", "amenity"];
const CONDITIONS = [
  "manual",
  "always",
  "has_pet",
  "is_military",
  "has_vehicle",
];
const CONDITION_LABEL: Record<string, string> = {
  manual: "Added by hand",
  always: "Every lease",
  has_pet: "Resident has a pet",
  is_military: "Resident is military",
  has_vehicle: "Resident has a vehicle",
};

export default function FeesPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("fee:manage");
  const allowed = can("fee:read") || manage;
  const fees = useQuery({
    queryKey: ["fees"],
    queryFn: () => api.fees(),
    enabled: scoped && allowed,
  });
  const { busy, run } = useRun([["fees"]]);
  const [adding, setAdding] = useState(false);

  const groups = useMemo(() => {
    const m = new Map<string, Fee[]>();
    for (const f of fees.data ?? []) {
      const list = m.get(f.kind) ?? [];
      list.push(f);
      m.set(f.kind, list);
    }
    return [...m.entries()].sort(
      ([a], [b]) => KINDS.indexOf(a) - KINDS.indexOf(b)
    );
  }, [fees.data]);

  if (!allowed) return <NoAccess what="the fee schedule" perm="fee:read" />;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Leasing"
        title="Fee schedule"
        description="Fees, discounts, rebates and amenities. Conditional items go on a lease by themselves when the resident qualifies."
        actions={
          manage && (
            <Button onClick={() => setAdding(true)}>
              <Plus />
              Add item
            </Button>
          )
        }
      />

      {fees.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load the fee schedule: {fees.error.message}
        </Panel>
      )}
      {fees.isLoading && <Skeleton className="h-64 rounded-2xl" />}

      {fees.data?.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Receipt />}
            title="No fees yet"
            description="Add pet rent, parking, a military discount or anything else you charge."
            action={
              manage && (
                <Button onClick={() => setAdding(true)}>
                  <Plus />
                  Add item
                </Button>
              )
            }
          />
        </Panel>
      )}

      {groups.map(([kind, list]) => (
        <Panel key={kind}>
          <PanelHeader
            title={`${humanize(kind)}s`}
            action={<Badge tone="neutral">{list.length}</Badge>}
          />
          <div className="overflow-x-auto p-2 pt-3">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="text-left">
                  <Th>Item</Th>
                  <Th>Applies when</Th>
                  <Th num>Amount</Th>
                  {manage && (
                    <th className="w-10">
                      <span className="sr-only">Remove</span>
                    </th>
                  )}
                </tr>
              </thead>
              <tbody>
                {list.map((f) => (
                  <tr key={f.id} className="border-t border-line align-top">
                    <td className="px-4 py-2.5">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium text-fg">{f.label}</span>
                        <span className="font-mono text-xs text-fg-3">
                          {f.code}
                        </span>
                        {!f.active && <Badge tone="neutral">inactive</Badge>}
                      </div>
                      {f.verbiage && (
                        <p className="mt-0.5 max-w-xl text-xs text-fg-3">
                          {f.verbiage}
                        </p>
                      )}
                    </td>
                    <td className="px-4 py-2.5">
                      <Badge
                        tone={
                          f.condition_type === "manual" ? "neutral" : "info"
                        }
                      >
                        {CONDITION_LABEL[f.condition_type] ?? f.condition_type}
                      </Badge>
                    </td>
                    <td className="figure px-4 py-2.5 text-right font-semibold whitespace-nowrap text-fg">
                      {f.amount_label}
                      {f.recurring ? "/mo" : ""}
                    </td>
                    {manage && (
                      <td className="px-2 py-1.5 text-right">
                        <button
                          type="button"
                          aria-label={`Remove ${f.label}`}
                          disabled={busy === f.id}
                          onClick={() => {
                            if (confirm(`Remove ${f.label} from the schedule?`))
                              void run(
                                f.id,
                                () => api.deleteFee(f.id),
                                "Removed"
                              );
                          }}
                          className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                        >
                          <Trash2 className="size-4" />
                        </button>
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      ))}

      {manage && <AddDialog open={adding} onOpenChange={setAdding} />}
    </div>
  );
}

function AddDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const { busy, run } = useRun([["fees"]]);
  const [code, setCode] = useState("");
  const [kind, setKind] = useState("fee");
  const [label, setLabel] = useState("");
  const [amount, setAmount] = useState("");
  const [condition, setCondition] = useState("manual");
  const [verbiage, setVerbiage] = useState("");
  const cents = toCents(amount);
  const valid = !!code.trim() && !!label.trim() && cents !== null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90dvh] overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          Add to the fee schedule
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Pick when it applies. Anything but &ldquo;added by hand&rdquo; goes on
          a lease when the fee schedule is applied.
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!valid || cents === null) return;
            const ok = await run(
              "add",
              () =>
                api.createFee({
                  code: code.trim(),
                  kind,
                  label: label.trim(),
                  amount_cents: cents,
                  condition_type: condition,
                  verbiage: verbiage.trim() || undefined,
                }),
              "Added to the schedule"
            );
            if (ok) {
              setCode("");
              setLabel("");
              setAmount("");
              setVerbiage("");
              onOpenChange(false);
            }
          }}
        >
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Kind">
              <select
                value={kind}
                onChange={(e) => setKind(e.target.value)}
                className={inputClass}
              >
                {KINDS.map((k) => (
                  <option key={k} value={k}>
                    {humanize(k)}
                  </option>
                ))}
              </select>
            </F>
            <F label="Code">
              <input
                value={code}
                onChange={(e) => setCode(e.target.value)}
                placeholder="pet_fee"
                className={`${inputClass} font-mono`}
                required
              />
            </F>
            <F label="Label">
              <input
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="Pet rent"
                className={inputClass}
                required
              />
            </F>
            <F label="Amount ($)">
              <input
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                inputMode="decimal"
                placeholder="50"
                className={inputClass}
                required
              />
            </F>
            <F label="Applies when" className="sm:col-span-2">
              <select
                value={condition}
                onChange={(e) => setCondition(e.target.value)}
                className={inputClass}
              >
                {CONDITIONS.map((c) => (
                  <option key={c} value={c}>
                    {CONDITION_LABEL[c]}
                  </option>
                ))}
              </select>
            </F>
            <F
              label="Lease wording (optional). You can use {amount}, {vehicles} and {pet_details}."
              className="sm:col-span-2"
            >
              <textarea
                value={verbiage}
                onChange={(e) => setVerbiage(e.target.value)}
                rows={3}
                className={inputClass}
              />
            </F>
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" loading={busy === "add"} disabled={!valid}>
              Add to schedule
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
