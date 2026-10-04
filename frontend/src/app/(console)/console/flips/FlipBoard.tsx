"use client";

// The acquisition board: a column per pipeline stage, a card per deal. The
// Flips page loads it lazily so its code only ships to people who open it.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { Plus, Search, TrendingUp } from "lucide-react";
import { toast } from "sonner";
import { api, type FlipDeal } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { F, FormDialog, input } from "@/components/property/bits";
import { pct, STRATEGIES, strategyLabel } from "./deal";

export default function FlipBoard() {
  const router = useRouter();
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const write = can("deal:write");
  const pipeline = useQuery({
    queryKey: ["flips"],
    queryFn: api.flipPipeline,
    enabled: scoped && can("deal:read"),
  });
  const [creating, setCreating] = useState(false);
  const [q, setQ] = useState("");

  const deals = useMemo(() => {
    const n = q.trim().toLowerCase();
    return (pipeline.data?.deals ?? []).filter(
      (d) =>
        !n ||
        d.name.toLowerCase().includes(n) ||
        d.address.toLowerCase().includes(n) ||
        d.city.toLowerCase().includes(n)
    );
  }, [pipeline.data, q]);
  const live = (pipeline.data?.deals ?? []).filter(
    (d) => d.stage !== "owned" && d.stage !== "dead"
  );
  const offered = live.reduce(
    (n, d) => n + (d.offer_price_cents ?? d.asking_price_cents ?? 0),
    0
  );
  const irrs = live
    .map((d) => d.underwriting.irr_pct)
    .filter((v): v is number => v !== null);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Deals"
        title="Acquisitions"
        description="Work buy-side deals from prospecting to close: underwrite each one, keep a data room, and turn a closed deal into an owned property."
        actions={
          write && (
            <Button onClick={() => setCreating(true)}>
              <Plus />
              New deal
            </Button>
          )
        }
      />

      {pipeline.data && (
        <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <Stat label="Live deals" value={live.length} />
          <Stat label="Offered or asking" value={usd(offered)} />
          <Stat
            label="Average IRR"
            value={
              irrs.length
                ? pct(irrs.reduce((a, b) => a + b, 0) / irrs.length)
                : "—"
            }
          />
          <Stat
            label="Owned"
            value={
              pipeline.data.deals.filter((d) => d.stage === "owned").length
            }
          />
        </section>
      )}

      {pipeline.isLoading && <Skeleton className="h-96 rounded-2xl" />}
      {pipeline.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load the pipeline: {pipeline.error.message}
        </Panel>
      )}

      {pipeline.data && pipeline.data.deals.length === 0 && (
        <Panel>
          <EmptyState
            icon={<TrendingUp />}
            title="No deals yet"
            description="Add a property you're looking at to start underwriting it."
            action={
              write && (
                <Button onClick={() => setCreating(true)}>
                  <Plus />
                  New deal
                </Button>
              )
            }
          />
        </Panel>
      )}

      {pipeline.data && pipeline.data.deals.length > 0 && (
        <>
          {pipeline.data.deals.length > 6 && (
            <div className="relative max-w-sm">
              <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Filter by name or address"
                aria-label="Filter deals"
                className="pl-9"
              />
            </div>
          )}
          <div className="-mx-1 overflow-x-auto px-1 pb-2">
            <div
              className="grid min-w-full gap-3"
              style={{
                gridTemplateColumns: `repeat(${pipeline.data.stages.length}, minmax(13rem, 1fr))`,
              }}
            >
              {pipeline.data.stages.map((s) => {
                const items = deals.filter((d) => d.stage === s.key);
                return (
                  <div key={s.key} className="space-y-2">
                    <div className="flex items-center justify-between px-1">
                      <h3 className="text-[13px] font-semibold text-fg">
                        {s.label}
                      </h3>
                      <span className="figure text-xs text-fg-3">
                        {items.length}
                      </span>
                    </div>
                    {items.map((d) => (
                      <DealCard key={d.id} deal={d} />
                    ))}
                    {items.length === 0 && (
                      <div className="flex min-h-20 items-center justify-center rounded-xl border border-dashed border-line text-xs text-fg-4">
                        Empty
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}

      {write && (
        <NewDeal
          open={creating}
          onOpenChange={setCreating}
          onCreated={(id) => router.push(`/console/flips/${id}`)}
        />
      )}
    </div>
  );
}

function DealCard({ deal: d }: { deal: FlipDeal }) {
  const price = d.offer_price_label ?? d.asking_price_label ?? "—";
  const u = d.underwriting;
  return (
    <Link
      href={`/console/flips/${d.id}`}
      className="block rounded-2xl outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      <Panel interactive className="space-y-2 p-3">
        <div className="flex items-start justify-between gap-2">
          <div className="text-[13px] leading-tight font-medium text-fg">
            {d.name}
          </div>
          <Badge className="shrink-0">{strategyLabel(d.strategy)}</Badge>
        </div>
        <div className="truncate text-xs text-fg-3">
          {[d.address, d.city].filter(Boolean).join(", ") || "No address"}
        </div>
        <div className="flex items-center justify-between gap-2 border-t border-line pt-2 text-xs">
          <span className="figure font-semibold text-fg">{price}</span>
          <span className="text-fg-3">
            Cap {pct(u.cap_rate_pct)} · IRR {pct(u.irr_pct)}
          </span>
        </div>
      </Panel>
    </Link>
  );
}

function NewDeal({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onCreated: (id: string) => void;
}) {
  const [name, setName] = useState("");
  const [address, setAddress] = useState("");
  const [city, setCity] = useState("");
  const [strategy, setStrategy] = useState("flip");
  const [asking, setAsking] = useState("");
  const [saving, setSaving] = useState(false);

  async function save() {
    if (!name.trim()) {
      toast.error("Give the deal a name");
      return;
    }
    setSaving(true);
    try {
      const cents = asking.trim()
        ? Math.round(Number(asking.replace(/[$,]/g, "")) * 100)
        : undefined;
      const deal = await api.createFlipDeal({
        name: name.trim(),
        address: address.trim() || undefined,
        city: city.trim() || undefined,
        strategy,
        asking_price_cents:
          cents !== undefined && !Number.isNaN(cents) ? cents : undefined,
      });
      onCreated(deal.id);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't create the deal");
      setSaving(false);
    }
  }

  return (
    <FormDialog
      open={open}
      onOpenChange={onOpenChange}
      title="New deal"
      busy={saving}
      onSave={save}
    >
      <F label="Deal name" className="sm:col-span-2">
        <input
          className={input}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Elm Street duplex"
          required
        />
      </F>
      <F label="Address">
        <input
          className={input}
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
      </F>
      <F label="City">
        <input
          className={input}
          value={city}
          onChange={(e) => setCity(e.target.value)}
        />
      </F>
      <F label="Strategy">
        <select
          className={input}
          value={strategy}
          onChange={(e) => setStrategy(e.target.value)}
        >
          {STRATEGIES.map((s) => (
            <option key={s.key} value={s.key}>
              {s.label}
            </option>
          ))}
        </select>
      </F>
      <F label="Asking price ($)">
        <input
          className={input}
          value={asking}
          onChange={(e) => setAsking(e.target.value)}
          inputMode="decimal"
          placeholder="285,000"
        />
      </F>
    </FormDialog>
  );
}
