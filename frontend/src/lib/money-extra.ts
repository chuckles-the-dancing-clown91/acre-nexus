"use client";

// Shared helpers for the money and time console pages (accounting, payments,
// payables, payouts, expenses, back office, timesheets, my time): dates in
// plain words, what a time entry was for, and the readiness gate.

import { useQuery } from "@tanstack/react-query";
import { api } from "./api";
import { useAuth } from "./auth";
import { queryKeys } from "./queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import {
  ENTRY_KIND_LABELS,
  type EntryKind,
  type Target,
  type TimeEntry,
} from "./backoffice";

/** The workspace is chosen and the viewer holds `perm`. */
export function useReady(perm?: string): boolean {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  return scoped && (!perm || can(perm));
}

export function errMsg(e: unknown, fallback = "Something went wrong"): string {
  return e instanceof Error ? e.message : fallback;
}

export function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

// ---- dates ------------------------------------------------------------------

export function addDays(d: Date, n: number): Date {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

/** `YYYY-MM-DD` as a local Date. */
export function parseYmd(s: string): Date {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/** `Mon, Oct 4` from an instant. */
export function fmtDay(iso: string): string {
  return new Date(iso).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}

/** `Oct 4, 2026` from `YYYY-MM-DD`. */
export function fmtDate(ymd: string): string {
  return parseYmd(ymd).toLocaleDateString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** `9:05 AM` from an instant. */
export function fmtTime(iso: string | null): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function fmtWhen(iso: string): string {
  return `${fmtDay(iso)} ${fmtTime(iso)}`;
}

/** Decimal hours from minutes: `7.50`. */
export function decimalHours(m: number): string {
  return (m / 60).toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function milesLabel(n: number): string {
  return n.toLocaleString("en-US", { maximumFractionDigits: 1 });
}

// ---- time entries -------------------------------------------------------------

export const OTHER_KINDS: EntryKind[] = ["travel", "shop", "admin", "other"];

/** What an entry was for, in plain words. */
export function workLabel(e: TimeEntry): string {
  if (e.kind === "work_order")
    return e.work_order_title ?? ENTRY_KIND_LABELS.work_order;
  if (e.kind === "project") return e.project_name ?? ENTRY_KIND_LABELS.project;
  if (e.kind === "property")
    return e.property_name ?? ENTRY_KIND_LABELS.property;
  return ENTRY_KIND_LABELS[e.kind];
}

/** Still on the clock (not ended, not waiting on a missed-punch review). */
export function isOpenEntry(e: TimeEntry): boolean {
  return !e.ended_at && !e.needs_review;
}

/** A target as one select value: `work_order:<id>`, `travel`, ... */
export function targetValue(t: Target): string {
  if (t.kind === "work_order") return `work_order:${t.maintenance_ticket_id}`;
  if (t.kind === "project") return `project:${t.rehab_project_id}`;
  if (t.kind === "property") return `property:${t.property_id}`;
  return t.kind;
}

export function targetFrom(v: string): Target | null {
  if (!v) return null;
  const [kind, id] = v.split(":");
  if (kind === "work_order")
    return { kind: "work_order", maintenance_ticket_id: id };
  if (kind === "project") return { kind: "project", rehab_project_id: id };
  if (kind === "property") return { kind: "property", property_id: id };
  return { kind: kind as EntryKind };
}

/** Distance from the job, for the "away" flag. */
export function distance(m: number | null): string {
  if (m == null) return "unknown";
  return m >= 160
    ? `${(m / 1609.34).toFixed(1)} mi`
    : `${Math.round(m * 3.281)} ft`;
}

// ---- entities -------------------------------------------------------------------

/**
 * Legal entities for a picker, defaulting to the one holding the most
 * properties (its books are the interesting ones).
 */
export function useEntityChoice(enabled: boolean) {
  const entities = useQuery({
    queryKey: queryKeys.llcs,
    queryFn: () => api.legalEntities(),
    enabled,
  });
  const groups = useQuery({
    queryKey: queryKeys.llcGroups,
    queryFn: () => api.llcGroups(),
    enabled,
  });
  const biggest =
    groups.data && groups.data.length > 0
      ? groups.data.reduce((a, b) =>
          b.property_count > a.property_count ? b : a
        )
      : undefined;
  const defaultId = biggest?.id ?? entities.data?.[0]?.id;
  return {
    entities: entities.data,
    loading: entities.isLoading,
    defaultId,
  };
}
