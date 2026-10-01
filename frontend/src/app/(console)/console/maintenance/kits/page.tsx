"use client";

// Job kits: the catalog a work order starts from. Each kit lists its tasks by
// trade (contractor work flagged) and the parts it takes, with an estimate.
// The company's managers add their own, change any, or copy one to start.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  ClipboardList,
  Copy,
  HardHat,
  Pencil,
  Plus,
  Search,
} from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useReach } from "@/components/shell/tenant-scope";
import { desk, tradeLabel } from "@/lib/servicedesk";
import { KitPreview } from "@/components/desk/KitPreview";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export default function KitsPage() {
  const { can } = useAuth();
  const { scoped } = useReach();
  // The catalog is the company's: people scoped to properties use it.
  const edit = can("maintenance:manage") && !scoped;
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  const [open, setOpen] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (kits.data ?? []).filter(
      (k) =>
        !needle ||
        k.name.toLowerCase().includes(needle) ||
        k.trades.some((t) => t.includes(needle))
    );
  }, [kits.data, q]);

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
        title="Job kits"
        description="Pick one on a new work order and its tasks, parts and estimate come with it."
        actions={
          <>
            {edit && (
              <Button variant="secondary" asChild>
                <Link href="/console/maintenance/kits/new">
                  <Plus />
                  New kit
                </Link>
              </Button>
            )}
            <Button asChild>
              <Link href="/console/maintenance/new">New work order</Link>
            </Button>
          </>
        }
      />
      <div className="relative max-w-sm">
        <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
        <Input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search by name or trade"
          aria-label="Search kits"
          className="pl-9"
        />
      </div>
      {kits.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {kits.data && rows.length === 0 && (
        <Panel>
          <EmptyState icon={<ClipboardList />} title="No kit matches" />
        </Panel>
      )}
      <div className="grid gap-3 lg:grid-cols-2">
        {rows.map((k) => (
          <Panel key={k.id} className="overflow-hidden">
            <button
              type="button"
              onClick={() => setOpen(open === k.id ? null : k.id)}
              aria-expanded={open === k.id}
              className="flex w-full items-start gap-3 p-4 text-left transition hover:bg-fill-2"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[15px] font-semibold text-fg">
                    {k.name}
                  </span>
                  {k.area && (
                    <span className="text-xs text-fg-3">{k.area}</span>
                  )}
                </div>
                <div className="mt-1 text-xs text-fg-3">
                  {k.tasks.length} tasks · {k.parts.length} parts · {k.priority}{" "}
                  priority
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {k.trades.map((t) => (
                    <Badge
                      key={t}
                      tone={
                        k.contractor_trades.includes(t) ? "warn" : "neutral"
                      }
                    >
                      {k.contractor_trades.includes(t) && (
                        <HardHat className="size-3" />
                      )}
                      {tradeLabel(t)}
                    </Badge>
                  ))}
                </div>
              </div>
              <span
                className={cn(
                  "figure text-[17px] font-semibold",
                  k.est_total_cents ? "text-fg" : "text-fg-4"
                )}
              >
                {k.est_total_cents ? k.est_total_label : "—"}
              </span>
            </button>
            {open === k.id && (
              <div className="border-t border-line p-4">
                <KitPreview kit={k} />
                {edit && (
                  <div className="mt-4 flex justify-end gap-2">
                    <Button size="sm" variant="ghost" asChild>
                      <Link href={`/console/maintenance/kits/new?from=${k.id}`}>
                        <Copy />
                        Copy
                      </Link>
                    </Button>
                    <Button size="sm" variant="secondary" asChild>
                      <Link href={`/console/maintenance/kits/${k.id}`}>
                        <Pencil />
                        Edit
                      </Link>
                    </Button>
                  </div>
                )}
              </div>
            )}
          </Panel>
        ))}
      </div>
    </div>
  );
}
