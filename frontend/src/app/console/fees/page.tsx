"use client";

import { useEffect, useState } from "react";
import { api, type Fee } from "@/lib/api";
import { Badge, Card } from "@/components/ui";
import { useAuth } from "@/lib/auth";

const KINDS = ["fee", "discount", "rebate", "amenity"];
const CONDITIONS = [
  "manual",
  "always",
  "has_pet",
  "is_military",
  "has_vehicle",
];

const CONDITION_LABEL: Record<string, string> = {
  manual: "Manual",
  always: "Always",
  has_pet: "If resident has a pet",
  is_military: "If resident is military",
  has_vehicle: "If resident has a vehicle",
};

export default function FeesPage() {
  const { can } = useAuth();
  const manage = can("fee:manage");
  const [fees, setFees] = useState<Fee[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Add form.
  const [code, setCode] = useState("");
  const [kind, setKind] = useState("fee");
  const [label, setLabel] = useState("");
  const [amount, setAmount] = useState("");
  const [condition, setCondition] = useState("manual");
  const [verbiage, setVerbiage] = useState("");
  const [busy, setBusy] = useState(false);

  const load = () =>
    api
      .fees()
      .then(setFees)
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
  }, []);

  async function add(e: React.FormEvent) {
    e.preventDefault();
    const dollars = parseFloat(amount);
    if (!code.trim() || !label.trim() || Number.isNaN(dollars)) return;
    setBusy(true);
    setError(null);
    try {
      await api.createFee({
        code: code.trim(),
        kind,
        label: label.trim(),
        amount_cents: Math.round(dollars * 100),
        condition_type: condition,
        verbiage: verbiage.trim() || undefined,
      });
      setCode("");
      setLabel("");
      setAmount("");
      setVerbiage("");
      load();
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    try {
      await api.deleteFee(id);
      load();
    } catch (err) {
      setError((err as Error).message);
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Fee schedule
        </h1>
        <p className="text-ink-3">
          Fees, discounts, rebates, and amenities. Conditional items
          auto-populate a lease from the resident&apos;s profile (pets,
          military, vehicles).
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {manage && (
        <Card className="p-5">
          <h2 className="mb-3 font-display text-lg font-bold">Add an item</h2>
          <form onSubmit={add} className="space-y-3">
            <div className="flex flex-wrap items-end gap-3">
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Kind</span>
                <select
                  value={kind}
                  onChange={(e) => setKind(e.target.value)}
                  className="rounded-lg border border-line bg-surface px-3 py-2 capitalize"
                >
                  {KINDS.map((k) => (
                    <option key={k} value={k}>
                      {k}
                    </option>
                  ))}
                </select>
              </label>
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Code</span>
                <input
                  value={code}
                  onChange={(e) => setCode(e.target.value)}
                  placeholder="pet_fee"
                  className="w-36 rounded-lg border border-line bg-surface px-3 py-2 font-mono text-xs"
                />
              </label>
              <label className="flex-1 min-w-[160px] text-sm">
                <span className="mb-1 block text-ink-3">Label</span>
                <input
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                  placeholder="Pet rent"
                  className="w-full rounded-lg border border-line bg-surface px-3 py-2"
                />
              </label>
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Amount $</span>
                <input
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  inputMode="decimal"
                  placeholder="50"
                  className="w-24 rounded-lg border border-line bg-surface px-3 py-2"
                />
              </label>
              <label className="text-sm">
                <span className="mb-1 block text-ink-3">Auto-apply when</span>
                <select
                  value={condition}
                  onChange={(e) => setCondition(e.target.value)}
                  className="rounded-lg border border-line bg-surface px-3 py-2"
                >
                  {CONDITIONS.map((c) => (
                    <option key={c} value={c}>
                      {CONDITION_LABEL[c]}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <label className="block text-sm">
              <span className="mb-1 block text-ink-3">
                Lease verbiage (optional) — supports {"{amount}"},{" "}
                {"{vehicles}"}, {"{pet_details}"}
              </span>
              <textarea
                value={verbiage}
                onChange={(e) => setVerbiage(e.target.value)}
                rows={2}
                className="w-full rounded-lg border border-line bg-surface px-3 py-2"
              />
            </label>
            <button
              type="submit"
              disabled={busy}
              className="rounded-lg bg-accent px-4 py-2 font-semibold text-white disabled:opacity-50"
            >
              Add to schedule
            </button>
          </form>
        </Card>
      )}

      <div className="space-y-3">
        {fees?.map((f) => (
          <Card key={f.id} className="flex flex-wrap items-center gap-3 p-4">
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <span className="font-semibold">{f.label}</span>
                <Badge tone="neutral">{f.kind}</Badge>
                <span className="font-mono text-xs text-ink-3">{f.code}</span>
              </div>
              {f.verbiage && (
                <p className="mt-1 text-sm text-ink-3">{f.verbiage}</p>
              )}
            </div>
            <Badge tone={f.condition_type === "manual" ? "neutral" : "info"}>
              {CONDITION_LABEL[f.condition_type] ?? f.condition_type}
            </Badge>
            <span className="font-mono text-sm font-bold">
              {f.amount_label}
              {f.recurring ? "/mo" : ""}
            </span>
            {manage && (
              <button onClick={() => remove(f.id)} className="text-ink-3">
                Remove
              </button>
            )}
          </Card>
        ))}
        {fees?.length === 0 && (
          <p className="text-ink-3">No fees configured yet.</p>
        )}
      </div>
    </div>
  );
}
