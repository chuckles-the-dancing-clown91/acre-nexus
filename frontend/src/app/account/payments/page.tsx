"use client";

// "My rent" — the resident's payment portal: balance and due items with
// one-click pay, saved (tokenized) payment methods, autopay enrollment, and
// receipt history. Payments ride the durable pipeline: an item flips to
// `processing` immediately and settles moments later (the page polls while
// anything is in flight).

import { useState } from "react";
import Link from "next/link";
import { api, type MyLease, type Payment } from "@/lib/api";
import { queryKeys, useMyLease } from "@/lib/queries";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Button, Card } from "@/components/ui";
import { useAuth } from "@/lib/auth";

function paymentTone(status: string) {
  switch (status) {
    case "paid":
      return "good" as const;
    case "failed":
    case "late":
      return "bad" as const;
    case "processing":
      return "info" as const;
    default:
      return "warn" as const;
  }
}

export default function MyPaymentsPage() {
  const { user, loading } = useAuth();
  const qc = useQueryClient();
  const { data: lease, error } = useMyLease({
    enabled: !!user,
    retry: false,
    // Poll while a payment is settling so the page follows it to paid.
    refetchInterval: (q) => {
      const d = q.state.data;
      const inFlight =
        d?.due_items.some((p) => p.status === "processing") ||
        d?.history.some((p) => p.status === "processing");
      return inFlight ? 3000 : false;
    },
  });
  const refresh = () => qc.invalidateQueries({ queryKey: queryKeys.myLease });

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[860px] px-6 py-8">
        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
          My rent
        </h1>
        <p className="mb-6 text-ink-3">
          Pay rent, manage payment methods, and set up autopay. Every payment
          issues a receipt.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">Sign in to manage your rent.</p>
            <Link
              href="/login"
              className="inline-block rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent"
            >
              Sign in
            </Link>
          </Card>
        )}

        {user && error && (
          <Card className="p-8 text-center text-ink-3">
            No active lease is linked to your account yet. Once you have a lease
            with us, your balance and payment options appear here.
          </Card>
        )}

        {user && lease && <LeasePanel lease={lease} onChange={refresh} />}
      </main>
    </>
  );
}

function LeasePanel({
  lease,
  onChange,
}: {
  lease: MyLease;
  onChange: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [methodId, setMethodId] = useState<string | undefined>(
    lease.methods.find((m) => m.autopay)?.id ?? lease.methods[0]?.id
  );
  const activeMethod = methodId ?? lease.methods[0]?.id;

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Request failed");
    } finally {
      setBusy(false);
    }
  }

  const payable = [...lease.due_items].sort((a, b) =>
    a.due_date.localeCompare(b.due_date)
  );

  return (
    <div className="space-y-5">
      <Card className="flex flex-wrap items-center justify-between gap-4 p-5">
        <div>
          <div className="font-display text-lg font-bold">
            {lease.property_name}
            {lease.unit_label ? ` · Unit ${lease.unit_label}` : ""}
          </div>
          <div className="text-sm text-ink-3">{lease.property_address}</div>
        </div>
        <div className="flex items-center gap-6">
          <div className="text-right">
            <div className="text-xs uppercase tracking-wide text-ink-3">
              Monthly rent
            </div>
            <div className="font-display text-xl font-extrabold">
              {lease.rent_label}
            </div>
          </div>
          <div className="text-right">
            <div className="text-xs uppercase tracking-wide text-ink-3">
              Balance due
            </div>
            <div
              className={`font-display text-xl font-extrabold ${lease.balance_cents > 0 ? "text-bad" : "text-good"}`}
            >
              {lease.balance_label}
            </div>
          </div>
          <Badge tone={paymentTone(lease.payment_status)}>
            {lease.payment_status}
          </Badge>
        </div>
      </Card>

      {/* Due items */}
      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Due now
        </div>
        <div className="divide-y divide-line">
          {payable.map((p) => (
            <DueRow
              key={p.id}
              payment={p}
              disabled={busy || !activeMethod}
              onPay={() =>
                activeMethod &&
                run(
                  () =>
                    api.payMyLease({
                      payment_id: p.id,
                      method_id: activeMethod,
                    }),
                  "Payment started — it settles in a few seconds"
                )
              }
            />
          ))}
          {lease.deposit_cents != null &&
            lease.deposit_cents > 0 &&
            !lease.deposit_paid && (
              <div className="flex items-center justify-between gap-3 px-5 py-3.5">
                <div>
                  <div className="font-semibold">Security deposit</div>
                  <div className="text-sm text-ink-3">
                    Held in escrow for the term of your lease.
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  <span className="font-mono">{lease.deposit_label}</span>
                  <Button
                    disabled={busy || !activeMethod}
                    onClick={() =>
                      activeMethod &&
                      run(
                        () =>
                          api.payMyLease({
                            kind: "deposit",
                            method_id: activeMethod,
                          }),
                        "Deposit payment started"
                      )
                    }
                  >
                    Pay deposit
                  </Button>
                </div>
              </div>
            )}
          {payable.length === 0 &&
            (lease.deposit_paid ||
              !lease.deposit_cents ||
              lease.deposit_cents <= 0) && (
              <div className="px-5 py-8 text-center text-ink-3">
                Nothing due — you&apos;re all caught up. 🎉
              </div>
            )}
        </div>
      </Card>

      {/* Methods + autopay */}
      <Card className="p-5">
        <div className="mb-3 flex items-center justify-between">
          <span className="font-display text-lg font-bold">
            Payment methods
          </span>
          {lease.autopay_enabled && lease.methods.some((m) => m.autopay) && (
            <Badge tone="good">
              Autopay on — day{" "}
              {lease.methods.find((m) => m.autopay)?.autopay_day ?? 1}
            </Badge>
          )}
        </div>
        <div className="space-y-2">
          {lease.methods.map((m) => (
            <div
              key={m.id}
              className={`flex flex-wrap items-center justify-between gap-3 rounded-xl border px-4 py-3 ${
                m.id === activeMethod ? "border-accent" : "border-line"
              }`}
            >
              <button
                className="flex items-center gap-3 text-left"
                onClick={() => setMethodId(m.id)}
              >
                <span className="font-semibold capitalize">
                  {m.brand ?? m.kind}
                </span>
                <span className="font-mono text-ink-3">•••• {m.last4}</span>
                {m.exp_month && m.exp_year && (
                  <span className="text-xs text-ink-3">
                    {m.exp_month}/{m.exp_year}
                  </span>
                )}
                {m.id === activeMethod && <Badge tone="accent">selected</Badge>}
              </button>
              <div className="flex gap-2">
                {lease.autopay_enabled &&
                  (m.autopay ? (
                    <Button
                      variant="outline"
                      disabled={busy}
                      onClick={() =>
                        run(() => api.cancelMyAutopay(), "Autopay cancelled")
                      }
                    >
                      Stop autopay
                    </Button>
                  ) : (
                    <Button
                      variant="outline"
                      disabled={busy}
                      onClick={() =>
                        run(
                          () => api.setMyAutopay(m.id),
                          "Autopay enrolled — rent charges automatically on its due date"
                        )
                      }
                    >
                      Enroll autopay
                    </Button>
                  ))}
                <Button
                  variant="ghost"
                  disabled={busy}
                  onClick={() =>
                    run(() => api.removeMyPaymentMethod(m.id), "Method removed")
                  }
                >
                  Remove
                </Button>
              </div>
            </div>
          ))}
        </div>
        <AddMethodForm busy={busy} run={run} />
      </Card>

      {/* History */}
      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Payment history
        </div>
        <div className="divide-y divide-line">
          {lease.history.map((p) => (
            <div
              key={p.id}
              className="flex items-center justify-between gap-3 px-5 py-3"
            >
              <div className="min-w-0">
                <span className="font-mono text-sm text-ink-3">
                  {p.paid_date ?? p.due_date}
                </span>
                <span className="ml-3 font-semibold capitalize">{p.kind}</span>
                {p.receipt_number && (
                  <span className="ml-3 font-mono text-xs text-ink-3">
                    {p.receipt_number}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-3">
                <span className="font-mono">{p.amount_label}</span>
                <Badge tone={paymentTone(p.status)}>{p.status}</Badge>
              </div>
            </div>
          ))}
          {lease.history.length === 0 && (
            <div className="px-5 py-8 text-center text-ink-3">
              No payments yet.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

function DueRow({
  payment,
  disabled,
  onPay,
}: {
  payment: Payment;
  disabled: boolean;
  onPay: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 px-5 py-3.5">
      <div>
        <div className="font-semibold capitalize">
          {payment.kind === "fee" ? "Fee" : "Rent"} — due {payment.due_date}
        </div>
        {payment.failure_reason && (
          <div className="text-sm text-bad">
            Last attempt failed: {payment.failure_reason}
          </div>
        )}
      </div>
      <div className="flex items-center gap-3">
        <span className="font-mono">{payment.amount_label}</span>
        <Badge tone={paymentTone(payment.status)}>{payment.status}</Badge>
        {payment.status !== "processing" && (
          <Button disabled={disabled} onClick={onPay}>
            {payment.status === "failed" ? "Retry" : "Pay now"}
          </Button>
        )}
      </div>
    </div>
  );
}

function AddMethodForm({
  busy,
  run,
}: {
  busy: boolean;
  run: (fn: () => Promise<unknown>, ok?: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<"card" | "ach">("card");
  const [last4, setLast4] = useState("");
  const [brand, setBrand] = useState("");

  if (!open) {
    return (
      <Button variant="outline" className="mt-3" onClick={() => setOpen(true)}>
        Add a payment method
      </Button>
    );
  }
  return (
    <div className="mt-4 rounded-xl border border-line bg-surface-2 p-4">
      <p className="mb-3 text-xs text-ink-3">
        Card and bank details are tokenized by the payment processor — only the
        last four digits are kept here.
      </p>
      <div className="flex flex-wrap items-end gap-3">
        <label className="text-sm font-semibold text-ink-2">
          Type
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value as "card" | "ach")}
            className="mt-1 block rounded-xl border border-line bg-surface px-3 py-2"
          >
            <option value="card">Card</option>
            <option value="ach">Bank (ACH)</option>
          </select>
        </label>
        <label className="text-sm font-semibold text-ink-2">
          {kind === "card" ? "Card number" : "Account number"}
          <input
            value={last4}
            onChange={(e) => setLast4(e.target.value)}
            placeholder={
              kind === "card" ? "4242 4242 4242 4242" : "000123456789"
            }
            className="mt-1 block w-56 rounded-xl border border-line bg-surface px-3 py-2 font-mono"
          />
        </label>
        {kind === "card" && (
          <label className="text-sm font-semibold text-ink-2">
            Brand
            <input
              value={brand}
              onChange={(e) => setBrand(e.target.value)}
              placeholder="Visa"
              className="mt-1 block w-28 rounded-xl border border-line bg-surface px-3 py-2"
            />
          </label>
        )}
        <Button
          disabled={busy || last4.replace(/\D/g, "").length < 4}
          onClick={() =>
            run(
              () =>
                api
                  .addMyPaymentMethod({
                    kind,
                    last4: last4.replace(/\D/g, ""),
                    brand: brand || undefined,
                  })
                  .then(() => setOpen(false)),
              "Payment method saved"
            )
          }
        >
          Save method
        </Button>
        <Button variant="ghost" onClick={() => setOpen(false)}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
