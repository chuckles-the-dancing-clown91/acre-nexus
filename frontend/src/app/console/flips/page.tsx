"use client";

// Acquisitions & Flips pipeline (preview module).
//
// Renders a kanban-style board over the real flip stage taxonomy. Deals are not
// yet populated by the backend (the `deal` domain lands next), so every column
// shows a tidy per-stage placeholder and the page surfaces a single EmptyState
// when the pipeline is empty overall. Built on the shared design system to match
// the dashboard's polish.

import {
  ArrowRight,
  Hammer,
  type LucideIcon,
  PackageCheck,
  Search,
  Sparkles,
  Tag,
  Workflow,
} from "lucide-react";
import { useFlipPipeline } from "@/lib/queries";
import { PageHeader, EmptyState } from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui";
import { cn } from "@/lib/utils";
import type { FlipStage } from "@/lib/api";

// A flip "deal" — the backend ships `deals: []` for now (typed `unknown[]`), so
// we describe the eventual shape locally and read it defensively. The moment the
// backend starts returning deals, the columns light up with zero page changes.
type FlipDeal = {
  id: string;
  stage: string;
  name?: string;
  address?: string;
  arv_label?: string;
  budget_label?: string;
  status?: string;
};

/** Iconography for the known stage taxonomy; falls back to a generic icon. */
const STAGE_ICONS: Record<string, LucideIcon> = {
  sourcing: Search,
  under_contract: Workflow,
  rehab: Hammer,
  listed: Tag,
  sold: PackageCheck,
};

function isFlipDeal(value: unknown): value is FlipDeal {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { stage?: unknown }).stage === "string" &&
    typeof (value as { id?: unknown }).id === "string"
  );
}

export default function FlipsPage() {
  const pipeline = useFlipPipeline();

  const stages = pipeline.data?.stages ?? [];
  const deals = (pipeline.data?.deals ?? []).filter(isFlipDeal);
  const dealsByStage = (stageKey: string) =>
    deals.filter((d) => d.stage === stageKey);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Acquisitions"
        title={
          <span className="inline-flex items-center gap-2.5">
            Acquisitions &amp; Flips
            <Badge tone="info">
              <Sparkles className="h-3 w-3" />
              Preview
            </Badge>
          </span>
        }
        description="Track buy/flip deals from sourcing through sale. The deal domain and underwriting tools land next."
      />

      {pipeline.isError ? (
        <EmptyState
          icon={Workflow}
          title="Couldn't load the pipeline"
          description={
            pipeline.error?.message ??
            "The flips module is enabled from Modules in settings. Try again in a moment."
          }
        />
      ) : pipeline.isLoading ? (
        <BoardSkeleton />
      ) : (
        <div className="space-y-5">
          {/* Board: one column per stage. */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-5">
            {stages.map((stage) => (
              <StageColumn
                key={stage.key}
                stage={stage}
                deals={dealsByStage(stage.key)}
              />
            ))}
          </div>

          {/* Overall empty state when no deals exist anywhere yet. */}
          {deals.length === 0 && (
            <EmptyState
              icon={Sparkles}
              title="No deals in the pipeline yet"
              description="This is a preview module. Once the deal domain ships, sourced acquisitions and active flips will flow through these stages."
            />
          )}
        </div>
      )}
    </div>
  );
}

/** One kanban column: a stage header with a deal count, then deal cards. */
function StageColumn({
  stage,
  deals,
}: {
  stage: FlipStage;
  deals: FlipDeal[];
}) {
  const Icon = STAGE_ICONS[stage.key] ?? Workflow;
  const isSold = stage.key === "sold";

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between px-0.5">
        <div className="flex items-center gap-2 text-ink">
          <span
            className={cn(
              "flex h-7 w-7 items-center justify-center rounded-lg",
              isSold
                ? "bg-good-soft text-good"
                : "bg-surface-2 text-ink-3"
            )}
          >
            <Icon className="h-4 w-4" />
          </span>
          <h3 className="font-display text-sm font-bold tracking-tight">
            {stage.label}
          </h3>
        </div>
        <span
          data-numeric
          className="rounded-full bg-surface-2 px-2 py-0.5 text-xs font-semibold text-ink-3"
        >
          {deals.length}
        </span>
      </div>

      {deals.length === 0 ? (
        <div className="flex min-h-28 flex-1 flex-col items-center justify-center rounded-xl border border-dashed border-line-2 bg-surface/40 px-3 py-6 text-center">
          <span className="text-xs text-ink-3">No deals</span>
        </div>
      ) : (
        <div className="space-y-3">
          {deals.map((deal) => (
            <DealCard key={deal.id} deal={deal} />
          ))}
        </div>
      )}
    </div>
  );
}

/** A single deal card. Renders defensively against the not-yet-final shape. */
function DealCard({ deal }: { deal: FlipDeal }) {
  return (
    <Card className="transition hover:shadow-acre-lg">
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">
          {deal.name ?? "Untitled deal"}
        </CardTitle>
        {deal.address && (
          <p className="truncate text-xs text-ink-3">{deal.address}</p>
        )}
      </CardHeader>
      <CardContent className="flex items-end justify-between pt-0">
        <div className="space-y-0.5">
          {deal.arv_label && (
            <div data-numeric className="text-sm font-semibold text-ink">
              {deal.arv_label}
            </div>
          )}
          {deal.budget_label && (
            <div className="text-xs text-ink-3">
              Budget{" "}
              <span data-numeric className="text-ink-2">
                {deal.budget_label}
              </span>
            </div>
          )}
        </div>
        {deal.status && (
          <Badge tone="neutral">
            {deal.status}
            <ArrowRight className="h-3 w-3" />
          </Badge>
        )}
      </CardContent>
    </Card>
  );
}

/** Loading scaffold mirroring the five-column board. */
function BoardSkeleton() {
  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-5">
      {Array.from({ length: 5 }).map((_, i) => (
        <div key={i} className="flex flex-col gap-3">
          <div className="flex items-center justify-between px-0.5">
            <div className="skeleton h-7 w-28 rounded-lg" />
            <div className="skeleton h-5 w-6 rounded-full" />
          </div>
          <div className="skeleton min-h-28 flex-1 rounded-xl" />
        </div>
      ))}
    </div>
  );
}
