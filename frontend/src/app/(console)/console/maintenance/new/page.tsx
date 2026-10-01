"use client";

// New work order: choose the property, pick a job kit ("Shower replacement")
// and the work order opens with its tasks by trade, the parts with typical
// costs, and the estimate. Or start blank.

import { Suspense, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft, HardHat, Search } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useProperties } from "@/lib/queries";
import { desk, tradeLabel, type Kit } from "@/lib/servicedesk";
import { KitPreview } from "@/components/desk/KitPreview";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function NewWorkOrderPage() {
  return (
    <Suspense fallback={<Skeleton className="h-96" />}>
      <NewWorkOrder />
    </Suspense>
  );
}

function NewWorkOrder() {
  const router = useRouter();
  const params = useSearchParams();
  const properties = useProperties();
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  const [propertyId, setPropertyId] = useState(params.get("property") ?? "");
  const [kitId, setKitId] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [note, setNote] = useState("");
  const [priority, setPriority] = useState("");
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);

  const kit = kits.data?.find((k) => k.id === kitId) ?? null;
  const groups = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const by = new Map<string, Kit[]>();
    for (const k of kits.data ?? []) {
      if (
        needle &&
        !k.name.toLowerCase().includes(needle) &&
        !(k.area ?? "").toLowerCase().includes(needle) &&
        !k.trades.some((t) => t.includes(needle))
      )
        continue;
      const list = by.get(k.category) ?? [];
      list.push(k);
      by.set(k.category, list);
    }
    return [...by.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [kits.data, q]);

  async function open() {
    if (!propertyId) {
      toast.error("Choose the property first.");
      return;
    }
    setBusy(true);
    try {
      if (kit) {
        const g = await desk.generate(kit.id, {
          property_id: propertyId,
          note: note.trim() || undefined,
          priority: priority || undefined,
        });
        router.push(`/console/maintenance/${g.ticket.id}`);
      } else {
        if (!title.trim()) {
          toast.error("Give the work order a title, or pick a kit.");
          setBusy(false);
          return;
        }
        const t = await api.createTicket(propertyId, {
          title: title.trim(),
          description: note.trim() || undefined,
          priority: priority || undefined,
        });
        router.push(`/console/maintenance/${t.id}`);
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't open it");
      setBusy(false);
    }
  }

  return (
    <div className="space-y-6">
      <Link
        href="/console/maintenance"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Service desk
      </Link>
      <PageHeader
        eyebrow="Service desk"
        title="New work order"
        description="Pick a job kit and the tasks, parts and estimate fill themselves in."
      />

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)]">
        <div className="space-y-4">
          <Panel className="p-5">
            <label className="block text-[13px] font-medium text-fg-2">
              Property
              <select
                className={cn(field, "mt-1.5")}
                value={propertyId}
                onChange={(e) => setPropertyId(e.target.value)}
              >
                <option value="">Choose a property…</option>
                {(properties.data ?? []).map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} · {p.address}
                  </option>
                ))}
              </select>
            </label>
          </Panel>

          <Panel>
            <PanelHeader
              title="Job kits"
              description="Each one lists the work by trade and the parts it takes."
              action={
                <div className="relative w-44 sm:w-56">
                  <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
                  <Input
                    value={q}
                    onChange={(e) => setQ(e.target.value)}
                    placeholder="Shower, toilet, paint…"
                    aria-label="Search kits"
                    className="pl-9"
                  />
                </div>
              }
            />
            <div className="space-y-5 p-5">
              {kits.isLoading && <Skeleton className="h-40" />}
              {groups.length === 0 && kits.data && (
                <EmptyState title="No kit matches" className="py-6" />
              )}
              {groups.map(([category, list]) => (
                <div key={category}>
                  <div className="eyebrow mb-2">{tradeLabel(category)}</div>
                  <div className="grid gap-2 sm:grid-cols-2">
                    {list.map((k) => (
                      <button
                        key={k.id}
                        type="button"
                        onClick={() => setKitId(kitId === k.id ? null : k.id)}
                        aria-pressed={kitId === k.id}
                        className={cn(
                          "rounded-xl border p-3 text-left transition",
                          kitId === k.id
                            ? "border-accent bg-accent/10"
                            : "border-line hover:border-line-strong hover:bg-fill-2"
                        )}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <span className="text-[14px] font-medium text-fg">
                            {k.name}
                          </span>
                          {k.est_total_cents > 0 && (
                            <span className="figure text-[13px] text-fg-2">
                              {k.est_total_label}
                            </span>
                          )}
                        </div>
                        <div className="mt-1 text-xs text-fg-3">
                          {k.tasks.length
                            ? `${k.tasks.length} tasks · ${k.parts.length} parts`
                            : `${k.parts.length} parts`}
                          {k.area ? ` · ${k.area}` : ""}
                        </div>
                        {k.trades.length > 0 && (
                          <div className="mt-2 flex flex-wrap gap-1">
                            {k.trades.slice(0, 5).map((t) => (
                              <Badge
                                key={t}
                                tone={
                                  k.contractor_trades.includes(t)
                                    ? "warn"
                                    : "neutral"
                                }
                              >
                                {k.contractor_trades.includes(t) && (
                                  <HardHat className="size-3" />
                                )}
                                {tradeLabel(t)}
                              </Badge>
                            ))}
                          </div>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </Panel>
        </div>

        <div className="space-y-4 xl:sticky xl:top-20 xl:self-start">
          <Panel>
            <PanelHeader
              title={kit ? kit.name : "Blank work order"}
              description={
                kit
                  ? "This is what the work order starts with. Everything can be changed after."
                  : "No kit: just a title and what's wrong."
              }
            />
            <div className="space-y-4 p-5">
              {kit ? (
                <KitPreview kit={kit} />
              ) : (
                <input
                  className={field}
                  placeholder="Title, e.g. Garage door won't close"
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                />
              )}
              <textarea
                className={cn(field, "min-h-[84px]")}
                placeholder="What was reported or seen"
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
              <select
                className={field}
                value={priority}
                onChange={(e) => setPriority(e.target.value)}
                aria-label="Priority"
              >
                <option value="">
                  Priority: {kit ? `kit default (${kit.priority})` : "normal"}
                </option>
                {["low", "normal", "high", "urgent"].map((p) => (
                  <option key={p} value={p}>
                    {p}
                  </option>
                ))}
              </select>
              <Button className="w-full" onClick={open} disabled={busy}>
                {busy ? "Opening…" : "Open work order"}
              </Button>
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}
