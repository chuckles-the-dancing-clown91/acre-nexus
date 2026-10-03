"use client";

// Rent: what's owed, paying a charge with a saved method, and what's been
// paid before.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CreditCard, Home } from "lucide-react";
import { toast } from "sonner";
import { api, ApiError } from "@/lib/api";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

export default function RentPage() {
  const qc = useQueryClient();
  const lease = useQuery({
    queryKey: ["my-lease"],
    queryFn: api.myLease,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  const [busy, setBusy] = useState<string | null>(null);

  if (lease.isLoading) return <Skeleton className="h-64" />;
  if (lease.error || !lease.data)
    return (
      <Panel>
        <EmptyState
          icon={<Home />}
          title="No lease on file yet"
          description="Once your lease is set up with this email, rent shows up here."
        />
      </Panel>
    );

  const l = lease.data;
  const method = l.methods.find((m) => m.status === "active") ?? l.methods[0];

  async function pay(paymentId: string) {
    if (!method) return;
    setBusy(paymentId);
    try {
      await api.payMyLease({ payment_id: paymentId, method_id: method.id });
      toast.success("Payment sent.");
      void qc.invalidateQueries({ queryKey: ["my-lease"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Payment didn't go through");
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Rent</h1>
        <p className="text-[13px] text-fg-3">
          {l.property_name}
          {l.unit_label && ` · ${l.unit_label}`}
        </p>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Panel className="p-4">
          <div className="eyebrow">Balance</div>
          <div className="figure mt-1 text-[24px] font-semibold text-fg">
            {l.balance_label}
          </div>
        </Panel>
        <Panel className="p-4">
          <div className="eyebrow">Monthly rent</div>
          <div className="figure mt-1 text-[24px] font-semibold text-fg">
            {l.rent_label}
          </div>
          {l.autopay_enabled && (
            <div className="mt-1 text-xs text-good">Autopay is on</div>
          )}
        </Panel>
      </div>

      <section>
        <div className="eyebrow mb-2">Due</div>
        <Panel className="divide-y divide-line">
          {l.due_items.length === 0 && (
            <p className="px-4 py-3 text-[13px] text-fg-3">
              Nothing due right now.
            </p>
          )}
          {l.due_items.map((p) => (
            <div key={p.id} className="flex items-center gap-3 px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="text-[14px] font-medium text-fg capitalize">
                  {p.kind.replace(/_/g, " ")}
                </div>
                <div className="text-xs text-fg-3">
                  Due {new Date(p.due_date + "T00:00").toLocaleDateString()}
                </div>
              </div>
              <span className="figure text-[14px] text-fg">
                {p.amount_label}
              </span>
              {method && (
                <Button
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => pay(p.id)}
                >
                  {busy === p.id ? "Paying…" : "Pay"}
                </Button>
              )}
            </div>
          ))}
        </Panel>
        {l.due_items.length > 0 && (
          <p className="mt-2 flex items-center gap-1.5 text-xs text-fg-3">
            <CreditCard className="size-3.5" />
            {method
              ? `Pays with ${method.brand ?? method.kind} ending ${method.last4}.`
              : "Add a card or bank account with the office to pay here."}
          </p>
        )}
      </section>

      {l.history.length > 0 && (
        <section>
          <div className="eyebrow mb-2">Paid</div>
          <Panel className="divide-y divide-line">
            {l.history.map((p) => (
              <div key={p.id} className="flex items-center gap-3 px-4 py-3">
                <div className="min-w-0 flex-1 text-[13px] text-fg capitalize">
                  {p.kind.replace(/_/g, " ")}
                  <span className="ml-2 text-xs text-fg-3 normal-case">
                    {p.paid_date
                      ? new Date(p.paid_date + "T00:00").toLocaleDateString()
                      : ""}
                  </span>
                </div>
                <span className="figure text-[13px] text-fg-2">
                  {p.amount_label}
                </span>
                <Badge tone={statusTone(p.status)}>{p.status}</Badge>
              </div>
            ))}
          </Panel>
        </section>
      )}
    </div>
  );
}
