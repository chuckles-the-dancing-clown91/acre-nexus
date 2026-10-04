// Small helpers shared by the resident pages for applications, lease,
// messages and profile: dates, words for statuses, and income parsing.

import type { Tone } from "@/components/ui/badge";

/** "2026-10-04" or an ISO instant → "Oct 4, 2026". Empty for nothing. */
export function day(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso.length === 10 ? `${iso}T00:00` : iso);
  if (Number.isNaN(d.getTime())) return iso.slice(0, 10);
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** An ISO instant → "Oct 4, 3:15 PM". */
export function when(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** The steps an application moves through, in order. */
export const APPLICATION_STEPS = [
  { key: "New", label: "Received" },
  { key: "Screening", label: "Screening" },
  { key: "Approved", label: "Approved" },
  { key: "Leased", label: "Leased" },
] as const;

/** What an application's status means for the applicant. */
export function applicationNote(status: string): string {
  switch (status) {
    case "New":
      return "We have your application. Screening starts soon and we'll email you at each step.";
    case "Screening":
      return "Your background check is running. We'll email you as soon as there's a decision.";
    case "Approved":
      return "You're approved. The leasing team will send your lease to sign online.";
    case "Leased":
      return "Your lease is ready. Check your email for the link to sign it.";
    case "Declined":
      return "We couldn't move forward with this application.";
    case "Withdrawn":
      return "This application was withdrawn.";
    default:
      return "";
  }
}

/** Tone for an application status. */
export function applicationTone(status: string): Tone {
  switch (status) {
    case "Approved":
    case "Leased":
      return "good";
    case "Screening":
      return "warn";
    case "Declined":
      return "bad";
    case "New":
      return "accent";
    default:
      return "neutral";
  }
}

/** Tone for an inspection checklist condition. */
export function conditionTone(condition: string): Tone {
  switch (condition) {
    case "good":
      return "good";
    case "fair":
      return "warn";
    case "unrated":
      return "neutral";
    default:
      return "bad";
  }
}

/** Words for a move-out settlement status. */
export function settlementWords(status: string): string {
  switch (status) {
    case "draft":
      return "Being prepared";
    case "processing":
      return "Refund on its way";
    case "closed":
      return "Settled";
    case "failed":
      return "Refund failed";
    default:
      return status.replace(/_/g, " ");
  }
}

/** Words for a document category. */
export function categoryWords(category: string | null): string {
  if (!category) return "Document";
  const s = category.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/**
 * Parse an annual income the way people type it ("52,000", "$52000.50").
 * Returns cents, `null` for blank, or `undefined` when it isn't a number.
 */
export function parseIncomeCents(raw: string): number | null | undefined {
  const s = raw.replace(/[$,\s]/g, "");
  if (!s) return null;
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return undefined;
  return Math.round(parseFloat(s) * 100);
}
