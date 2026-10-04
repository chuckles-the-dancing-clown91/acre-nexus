"use client";

// Rent and the charges on top of it: fees, discounts, rebates and amenities.
// Managers add one by hand, remove one, or apply the fee schedule, which adds
// whatever the resident's profile qualifies for (pets, military, vehicles).

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Plus, Sparkles, X } from "lucide-react";
import { api } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, inputClass, toCents, useRun } from "../_ui/shared";

const KINDS = ["fee", "discount", "rebate", "amenity"];

export function Charges({
  leaseId,
  manage,
}: {
  leaseId: string;
  manage: boolean;
}) {
  const charges = useQuery({
    queryKey: ["leases", leaseId, "charges"],
    queryFn: () => api.leaseCharges(leaseId),
  });
  const { busy, run } = useRun([["leases", leaseId]]);
  const [adding, setAdding] = useState(false);
  const [kind, setKind] = useState("fee");
  const [label, setLabel] = useState("");
  const [amount, setAmount] = useState("");

  const cents = toCents(amount);
  const c = charges.data;

  return (
    <Panel>
      <PanelHeader
        title="Rent and charges"
        description={c ? `${c.monthly_total_label} a month, all in` : undefined}
        action={
          manage && (
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="secondary"
                loading={busy === "apply"}
                onClick={() =>
                  run(
                    "apply",
                    () => api.applyFees(leaseId),
                    "Fee schedule applied"
                  )
                }
              >
                <Sparkles />
                Apply fee schedule
              </Button>
              {!adding && (
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => setAdding(true)}
                >
                  <Plus />
                  Add
                </Button>
              )}
            </div>
          )
        }
      />
      <div className="p-2 pt-3">
        {charges.isLoading && <Skeleton className="m-3 h-20" />}
        {charges.error && (
          <p className="px-3 py-2 text-[13px] text-bad">
            Couldn&apos;t load charges: {charges.error.message}
          </p>
        )}
        {c && (
          <ul className="divide-y divide-line">
            <li className="flex items-center gap-3 px-3 py-2.5 text-[13px]">
              <span className="flex-1 font-medium text-fg">Base rent</span>
              <span className="figure text-fg">{c.base_rent_label}</span>
              {manage && <span className="w-7" />}
            </li>
            {c.charges.map((ch) => {
              const negative = ch.amount_cents < 0;
              return (
                <li
                  key={ch.id}
                  className="flex items-center gap-3 px-3 py-2.5 text-[13px]"
                >
                  <span className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
                    <span className="text-fg-2">{ch.label}</span>
                    <Badge tone="neutral">{ch.kind}</Badge>
                    {ch.source === "auto" && <Badge tone="info">auto</Badge>}
                  </span>
                  <span
                    className={negative ? "figure text-good" : "figure text-fg"}
                  >
                    {negative ? "−" : "+"}
                    {ch.amount_label.replace("-", "")}
                    {ch.recurring ? "/mo" : ""}
                  </span>
                  {manage && (
                    <button
                      type="button"
                      aria-label={`Remove ${ch.label}`}
                      disabled={busy === `del-${ch.id}`}
                      onClick={() =>
                        run(`del-${ch.id}`, () => api.deleteLeaseCharge(ch.id))
                      }
                      className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                    >
                      <X className="size-4" />
                    </button>
                  )}
                </li>
              );
            })}
            <li className="flex items-center gap-3 border-t-2 border-line px-3 py-2.5 text-[13px] font-semibold">
              <span className="flex-1 text-fg">Monthly total</span>
              <span className="figure text-fg">{c.monthly_total_label}</span>
              {manage && <span className="w-7" />}
            </li>
          </ul>
        )}
      </div>

      {manage && adding && (
        <form
          className="mx-5 mb-5 grid gap-3 rounded-xl border border-line bg-fill/40 p-3 sm:grid-cols-[auto_1fr_auto]"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!label.trim() || cents === null) return;
            const ok = await run("add", () =>
              api.addLeaseCharge(leaseId, {
                kind,
                label: label.trim(),
                amount_cents: cents,
              })
            );
            if (ok) {
              setLabel("");
              setAmount("");
              setAdding(false);
            }
          }}
        >
          <F label="Kind">
            <select
              value={kind}
              onChange={(e) => setKind(e.target.value)}
              className={`${inputClass} capitalize`}
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </F>
          <F label="Label">
            <input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="Parking space"
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
              className={`${inputClass} sm:w-28`}
              required
            />
          </F>
          <div className="flex justify-end gap-2 sm:col-span-3">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => setAdding(false)}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              loading={busy === "add"}
              disabled={!label.trim() || cents === null}
            >
              Add charge
            </Button>
          </div>
        </form>
      )}
    </Panel>
  );
}
