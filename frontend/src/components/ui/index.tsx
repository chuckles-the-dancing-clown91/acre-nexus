// Modular, pluggable UI primitives. Each is presentational and theme-aware
// (colours come from CSS variables), so they can be dropped into any page or
// composed into larger components.

import { clsx } from "@/lib/clsx";
import { Icon } from "@/components/Icon";

// ---- Card ------------------------------------------------------------------
export function Card({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={clsx(
        "rounded-2xl border border-line bg-surface shadow-acre",
        className
      )}
    >
      {children}
    </div>
  );
}

// ---- Badge -----------------------------------------------------------------
type Tone = "neutral" | "good" | "warn" | "bad" | "info" | "accent";

const TONES: Record<Tone, string> = {
  neutral: "bg-surface-2 text-ink-2",
  good: "bg-good-soft text-good",
  warn: "bg-warn-soft text-warn",
  bad: "bg-bad-soft text-bad",
  info: "bg-info-soft text-info",
  accent: "bg-accent-soft text-accent-2",
};

export function Badge({
  tone = "neutral",
  children,
  className,
}: {
  tone?: Tone;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-bold",
        TONES[tone],
        className
      )}
    >
      {children}
    </span>
  );
}

/** Map a status string to a sensible badge tone. */
export function statusTone(status: string): Tone {
  const s = status.toLowerCase();
  if (s === "stabilized" || s === "current" || s === "available" || s === "approved")
    return "good";
  if (s === "vacant" || s.includes("vacant") || s === "new") return "accent";
  if (s === "late" || s === "declined") return "bad";
  if (s === "notice" || s === "pending" || s === "screening") return "warn";
  return "neutral";
}

// ---- Button ----------------------------------------------------------------
export function Button({
  variant = "primary",
  className,
  children,
  ...rest
}: {
  variant?: "primary" | "ghost" | "outline";
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const styles =
    variant === "primary"
      ? "bg-accent text-on-accent hover:opacity-90"
      : variant === "outline"
      ? "border border-line-2 bg-surface text-ink hover:bg-surface-2"
      : "text-ink-2 hover:bg-surface-2";
  return (
    <button
      {...rest}
      className={clsx(
        "inline-flex items-center justify-center gap-2 rounded-xl px-4 py-2.5 text-sm font-bold transition disabled:opacity-50",
        styles,
        className
      )}
    >
      {children}
    </button>
  );
}

// ---- StatTile --------------------------------------------------------------
export function StatTile({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon?: string;
}) {
  return (
    <Card className="p-4">
      <div className="mb-2 flex items-center justify-between text-ink-3">
        <span className="text-xs font-semibold uppercase tracking-wide">
          {label}
        </span>
        {icon && <Icon name={icon} size={16} />}
      </div>
      <div className="font-display text-2xl font-extrabold tracking-tight">
        {value}
      </div>
    </Card>
  );
}
