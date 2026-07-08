"use client";

// Acquisition deal board (kanban-style columns over the pipeline stages). The
// heavy, module-specific component the Flips page loads lazily via
// `next/dynamic`, so its code only ships to tenants who open the module.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api, type FlipPipeline, type FlipDeal } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";
import { logError } from "@/lib/log";

const STRATEGIES = [
  { key: "flip", label: "Fix & flip" },
  { key: "brrrr", label: "BRRRR" },
  { key: "rental", label: "Buy & hold rental" },
  { key: "hold", label: "Land / hold" },
  { key: "wholesale", label: "Wholesale" },
];

function pct(v: number | null): string {
  return v === null ? "—" : `${v.toFixed(1)}%`;
}

function DealCard({ deal }: { deal: FlipDeal }) {
  const price = deal.offer_price_label ?? deal.asking_price_label ?? "—";
  const u = deal.underwriting;
  return (
    <Link href={`/console/flips/${deal.id}`} className="block">
      <Card className="space-y-2 p-3 transition hover:border-line-2">
        <div className="flex items-start justify-between gap-2">
          <div className="font-display text-sm font-bold leading-tight">
            {deal.name}
          </div>
          <Badge tone="neutral">{deal.strategy}</Badge>
        </div>
        <div className="text-xs text-ink-3">
          {[deal.address, deal.city].filter(Boolean).join(", ") || "—"}
        </div>
        <div className="flex items-center justify-between border-t border-line pt-2 text-xs">
          <span className="font-bold text-ink">{price}</span>
          <span className="text-ink-3">
            Cap {pct(u.cap_rate_pct)} · IRR {pct(u.irr_pct)}
          </span>
        </div>
      </Card>
    </Link>
  );
}

function NewDealForm({
  onCreated,
  onCancel,
}: {
  onCreated: (id: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [city, setCity] = useState("");
  const [strategy, setStrategy] = useState("flip");
  const [asking, setAsking] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setError("Give the deal a name.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const askingCents = asking.trim()
        ? Math.round(Number(asking) * 100)
        : undefined;
      const deal = await api.createFlipDeal({
        name: name.trim(),
        address: address.trim() || undefined,
        city: city.trim() || undefined,
        strategy,
        asking_price_cents:
          askingCents !== undefined && !Number.isNaN(askingCents)
            ? askingCents
            : undefined,
      });
      onCreated(deal.id);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Couldn't create the deal.";
      setError(msg);
      logError("failed to create deal", e);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card className="p-4">
      <form onSubmit={submit} className="grid gap-3 sm:grid-cols-2">
        <label className="text-sm sm:col-span-2">
          <span className="mb-1 block font-semibold text-ink-2">Deal name</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Elm Street Duplex"
            className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block font-semibold text-ink-2">Address</span>
          <input
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block font-semibold text-ink-2">City</span>
          <input
            value={city}
            onChange={(e) => setCity(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
          />
        </label>
        <label className="text-sm">
          <span className="mb-1 block font-semibold text-ink-2">Strategy</span>
          <select
            value={strategy}
            onChange={(e) => setStrategy(e.target.value)}
            className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
          >
            {STRATEGIES.map((s) => (
              <option key={s.key} value={s.key}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        <label className="text-sm">
          <span className="mb-1 block font-semibold text-ink-2">
            Asking price ($)
          </span>
          <input
            value={asking}
            onChange={(e) => setAsking(e.target.value)}
            inputMode="decimal"
            placeholder="285000"
            className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
          />
        </label>
        {error && <div className="text-sm text-bad sm:col-span-2">{error}</div>}
        <div className="flex gap-2 sm:col-span-2">
          <Button type="submit" disabled={saving}>
            {saving ? "Creating…" : "Create deal"}
          </Button>
          <Button type="button" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        </div>
      </form>
    </Card>
  );
}

export default function FlipBoard() {
  const router = useRouter();
  const { can } = useAuth();
  const [data, setData] = useState<FlipPipeline | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const load = useCallback(() => {
    api
      .flipPipeline()
      .then(setData)
      .catch((e) => setError(e.message));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  if (error) {
    return (
      <div className="rounded-xl border border-bad-soft bg-bad-soft/40 px-4 py-3 text-sm text-bad">
        {error}
      </div>
    );
  }
  if (!data) return <div className="text-ink-3">Loading pipeline…</div>;

  const byStage = (key: string) => data.deals.filter((d) => d.stage === key);

  return (
    <div className="space-y-4">
      {can("deal:write") && (
        <div>
          {creating ? (
            <NewDealForm
              onCreated={(id) => router.push(`/console/flips/${id}`)}
              onCancel={() => setCreating(false)}
            />
          ) : (
            <Button onClick={() => setCreating(true)}>+ New deal</Button>
          )}
        </div>
      )}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-6">
        {data.stages.map((stage) => {
          const deals = byStage(stage.key);
          return (
            <div key={stage.key} className="space-y-2">
              <div className="flex items-center justify-between px-1">
                <h3 className="font-display text-sm font-bold">
                  {stage.label}
                </h3>
                <span className="text-xs text-ink-3">{deals.length}</span>
              </div>
              <div className="space-y-2">
                {deals.length === 0 ? (
                  <Card className="flex min-h-20 items-center justify-center p-3 text-center text-xs text-ink-3">
                    —
                  </Card>
                ) : (
                  deals.map((d) => <DealCard key={d.id} deal={d} />)
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
