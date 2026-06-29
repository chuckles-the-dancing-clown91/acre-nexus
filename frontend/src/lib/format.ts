import { format, formatDistanceToNow, isValid } from "date-fns";

/**
 * Pure formatting helpers (no React). All guard against null / NaN / invalid
 * input so callers never have to. Currency is USD with no decimals to match the
 * `*_label` strings the API returns.
 */

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

/** Cents → "$1,234". Null/undefined/NaN render as "$0". */
export function currencyFromCents(cents: number | null | undefined): string {
  if (cents == null || !Number.isFinite(cents)) return "$0";
  return usd.format(Math.round(cents) / 100);
}

/** Dollar amount → "$1,234" (USD, no decimals). */
export function currency(amount: number): string {
  if (!Number.isFinite(amount)) return "$0";
  return usd.format(amount);
}

function toDate(iso: string | null | undefined): Date | null {
  if (!iso) return null;
  const d = new Date(iso);
  return isValid(d) ? d : null;
}

/** ISO string → formatted date (default "MMM d, yyyy"). Empty string if invalid. */
export function formatDate(
  iso: string | null | undefined,
  fmt: string = "MMM d, yyyy",
): string {
  const d = toDate(iso);
  return d ? format(d, fmt) : "";
}

/** ISO string → "MMM d, yyyy · h:mm a". Empty string if invalid. */
export function formatDateTime(iso: string | null | undefined): string {
  const d = toDate(iso);
  return d ? format(d, "MMM d, yyyy · h:mm a") : "";
}

/** ISO string → relative phrase, e.g. "3 days ago". Empty string if invalid. */
export function relativeDate(iso: string | null | undefined): string {
  const d = toDate(iso);
  return d ? formatDistanceToNow(d, { addSuffix: true }) : "";
}

/** Number → "95%". Null/undefined/NaN render as "0%". */
export function percent(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return "0%";
  return `${Math.round(n)}%`;
}

/** Name → up to 2 uppercase initials, e.g. "Jane Doe" → "JD". */
export function initials(name: string): string {
  if (!name) return "";
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "";
  const picks =
    parts.length === 1 ? [parts[0]] : [parts[0], parts[parts.length - 1]];
  return picks
    .map((p) => p[0]?.toUpperCase() ?? "")
    .join("")
    .slice(0, 2);
}

/** Lower/mixed text → Title Case, e.g. "hello world" → "Hello World". */
export function titleCase(s: string): string {
  if (!s) return "";
  return s
    .toLowerCase()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}
