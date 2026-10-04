// Small pieces the CRM tabs share.

import { isoDate, type LeadStatus, type SubjectType } from "@/lib/backoffice";
import type { Tone } from "@/components/ui/badge";
import { parseDay } from "./timeline";

/** Whatever the side panel has open. */
export type Opened = { type: SubjectType; id: string; name: string | null };

export const OWNER_KINDS = ["individual", "company", "firm"] as const;

export const SUBJECT_LABEL: Record<SubjectType, string> = {
  owner: "Owner",
  owner_lead: "Lead",
  counterparty: "Vendor",
  property: "Property",
};

export const STAGE_LABEL: Record<LeadStatus, string> = {
  new: "New",
  contacted: "Contacted",
  proposal: "Proposal sent",
  won: "Won",
  lost: "Lost",
};

export function stageTone(s: LeadStatus): Tone {
  switch (s) {
    case "new":
      return "info";
    case "contacted":
      return "accent";
    case "proposal":
      return "warn";
    case "won":
      return "good";
    default:
      return "bad";
  }
}

export function humanize(key: string): string {
  return key.charAt(0).toUpperCase() + key.slice(1).replace(/_/g, " ");
}

export function errMsg(e: unknown, fallback: string) {
  return e instanceof Error ? e.message : fallback;
}

/** "3 days ago" for an instant. */
export function ago(iso: string | null): string {
  if (!iso) return "Never";
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
  const units: [number, string][] = [
    [60 * 60 * 24 * 365, "year"],
    [60 * 60 * 24 * 30, "month"],
    [60 * 60 * 24 * 7, "week"],
    [60 * 60 * 24, "day"],
    [60 * 60, "hour"],
    [60, "minute"],
  ];
  for (const [size, name] of units) {
    const n = Math.floor(s / size);
    if (n >= 1) return `${n} ${name}${n === 1 ? "" : "s"} ago`;
  }
  return "Just now";
}

/** `YYYY-MM-DD`, `days` after the later of today and `from`. */
export function snoozeDay(from: string | null, days: number): string {
  const today = isoDate(new Date());
  const base = parseDay(from && from > today ? from : today);
  base.setDate(base.getDate() + days);
  return isoDate(base);
}

/** A small labelled figure for the side panel. */
export function MiniStat({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="min-w-0 rounded-xl border border-line bg-fill px-3 py-2.5">
      <div className="eyebrow">{label}</div>
      <div className="figure mt-1 truncate text-[17px] font-semibold text-fg">
        {value}
      </div>
      {sub && <div className="truncate text-[11px] text-fg-3">{sub}</div>}
    </div>
  );
}
