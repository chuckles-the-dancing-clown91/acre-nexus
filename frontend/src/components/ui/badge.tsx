import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

export type Tone =
  "neutral" | "accent" | "good" | "warn" | "bad" | "info" | "plasma";

export const badgeVariants = cva(
  "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium whitespace-nowrap",
  {
    variants: {
      tone: {
        neutral: "border-line-strong bg-fill text-fg-2",
        accent: "border-accent/25 bg-accent/12 text-accent",
        good: "border-good/25 bg-good/12 text-good",
        warn: "border-warn/25 bg-warn/12 text-warn",
        bad: "border-bad/25 bg-bad/12 text-bad",
        info: "border-info/25 bg-info/12 text-info",
        plasma: "border-plasma/25 bg-plasma/12 text-plasma",
      },
    },
    defaultVariants: { tone: "neutral" },
  }
);

export function Badge({
  tone,
  dot,
  className,
  children,
}: VariantProps<typeof badgeVariants> & {
  dot?: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <span className={cn(badgeVariants({ tone }), className)}>
      {dot && <span className="size-1.5 rounded-full bg-current" />}
      {children}
    </span>
  );
}

const TONE_TEXT: Record<Tone, string> = {
  neutral: "text-fg-3",
  accent: "text-accent",
  good: "text-good",
  warn: "text-warn",
  bad: "text-bad",
  info: "text-info",
  plasma: "text-plasma",
};

export function toneText(tone: Tone) {
  return TONE_TEXT[tone];
}

/** A small status light. Pulses in HUD mode when `live`. */
export function StatusDot({
  tone = "good",
  live,
  className,
}: {
  tone?: Tone;
  live?: boolean;
  className?: string;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-2 shrink-0 rounded-full bg-current",
        TONE_TEXT[tone],
        live && "hud:animate-[telemetry-pulse_2s_ease-out_infinite]",
        className
      )}
    />
  );
}

/** Map a backend status string to a tone. */
export function statusTone(status: string): Tone {
  const s = status.toLowerCase();
  if (
    [
      "stabilized",
      "current",
      "available",
      "approved",
      "active",
      "paid",
      "resolved",
      "closed",
    ].includes(s)
  )
    return "good";
  if (s.includes("vacant") || s === "new" || s === "open") return "accent";
  if (["late", "declined", "failed", "urgent", "overdue"].includes(s))
    return "bad";
  if (
    ["notice", "pending", "screening", "on_hold", "partial", "due"].includes(s)
  )
    return "warn";
  if (["scheduled", "in_progress", "triage"].includes(s)) return "info";
  return "neutral";
}
