import * as React from "react";
import { cn } from "@/lib/utils";
import { Card } from "@/components/ui/card";
import type { LucideIcon } from "lucide-react";

/** Standard page header: eyebrow + title + description on the left, actions right. */
export function PageHeader({
  title,
  description,
  eyebrow,
  actions,
  className,
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  eyebrow?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between",
        className
      )}
    >
      <div className="min-w-0">
        {eyebrow && (
          <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-3">
            {eyebrow}
          </div>
        )}
        <h1 className="truncate font-display text-2xl font-bold tracking-tight text-ink">
          {title}
        </h1>
        {description && (
          <p className="mt-1 text-sm text-ink-2">{description}</p>
        )}
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-2">{actions}</div>
      )}
    </div>
  );
}

const STAT_TONES = {
  neutral: "text-ink",
  good: "text-good",
  warn: "text-warn",
  bad: "text-bad",
  accent: "text-accent-2",
} as const;

/** A KPI tile: label, big tabular value, optional sub-line and trend. */
export function StatCard({
  label,
  value,
  sub,
  icon: Icon,
  tone = "neutral",
  className,
}: {
  label: string;
  value: React.ReactNode;
  sub?: React.ReactNode;
  icon?: LucideIcon;
  tone?: keyof typeof STAT_TONES;
  className?: string;
}) {
  return (
    <Card className={cn("p-4", className)}>
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          {label}
        </span>
        {Icon && <Icon className="h-4 w-4 text-ink-3" />}
      </div>
      <div
        data-numeric
        className={cn(
          "mt-2 font-display text-3xl font-bold tracking-tight",
          STAT_TONES[tone]
        )}
      >
        {value}
      </div>
      {sub && <div className="mt-1 text-xs text-ink-2">{sub}</div>}
    </Card>
  );
}

/** Empty / zero-state placeholder. */
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
}: {
  icon?: LucideIcon;
  title: string;
  description?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center rounded-xl border border-dashed border-line-2 bg-surface/50 px-6 py-14 text-center",
        className
      )}
    >
      {Icon && (
        <div className="mb-3 flex h-11 w-11 items-center justify-center rounded-xl bg-surface-2 text-ink-3">
          <Icon className="h-5 w-5" />
        </div>
      )}
      <h3 className="font-display text-sm font-semibold text-ink">{title}</h3>
      {description && (
        <p className="mt-1 max-w-sm text-sm text-ink-2">{description}</p>
      )}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}
