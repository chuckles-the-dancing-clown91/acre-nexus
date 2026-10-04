"use client";

// Small pieces the workspace setup screens share: a copy button, an on/off
// switch, star ratings, a key/value row, and a one-time secret callout.

import { useState, useSyncExternalStore } from "react";
import { Check, Copy, Star, TriangleAlert } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function errorText(e: unknown, fallback = "Something went wrong") {
  return e instanceof Error && e.message ? e.message : fallback;
}

/** Copies `value` to the clipboard and says so. */
export function CopyButton({
  value,
  label = "Copy",
  size = "sm",
  variant = "secondary",
  className,
}: {
  value: string;
  label?: string;
  size?: "sm" | "md" | "icon";
  variant?: "secondary" | "ghost" | "primary";
  className?: string;
}) {
  const [done, setDone] = useState(false);
  return (
    <Button
      type="button"
      size={size}
      variant={variant}
      className={className}
      aria-label={size === "icon" ? label : undefined}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(value);
          setDone(true);
          toast.success("Copied");
          window.setTimeout(() => setDone(false), 1500);
        } catch {
          toast.error("Couldn't copy. Select the text and copy it instead.");
        }
      }}
    >
      {done ? <Check /> : <Copy />}
      {size !== "icon" && (done ? "Copied" : label)}
    </Button>
  );
}

/** An accessible on/off switch, styled like the settings page's. */
export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full transition disabled:opacity-45",
        checked ? "bg-accent" : "bg-fill-2"
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 size-5 rounded-full bg-white shadow transition",
          checked ? "left-[22px]" : "left-0.5"
        )}
      />
    </button>
  );
}

/** Five stars, filled to the rating. */
export function Stars({
  rating,
  size = 14,
}: {
  rating: number;
  size?: number;
}) {
  return (
    <span
      className="inline-flex items-center gap-0.5"
      aria-label={`${rating.toFixed(1)} out of 5`}
    >
      {[1, 2, 3, 4, 5].map((n) => (
        <Star
          key={n}
          style={{ width: size, height: size }}
          className={cn(
            n <= Math.round(rating) ? "fill-warn text-warn" : "text-fg-4"
          )}
        />
      ))}
    </span>
  );
}

/** One label/value line in a definition list. */
export function Row({
  label,
  value,
  mono = true,
  copy,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
  copy?: string;
}) {
  return (
    <div className="flex flex-col gap-1 border-b border-line py-2.5 last:border-0 sm:flex-row sm:items-center sm:gap-4">
      <dt className="w-48 shrink-0 text-[13px] text-fg-3">{label}</dt>
      <dd
        className={cn(
          "min-w-0 flex-1 break-all text-fg",
          mono ? "font-mono text-xs" : "text-[13px]"
        )}
      >
        {value}
      </dd>
      {copy && <CopyButton value={copy} size="icon" variant="ghost" />}
    </div>
  );
}

/** A secret shown once, with a copy button and a dismiss. */
export function SecretOnce({
  title = "Copy this now. It is not shown again.",
  secret,
  onDismiss,
  children,
}: {
  title?: string;
  secret: string;
  onDismiss?: () => void;
  children?: React.ReactNode;
}) {
  return (
    <div className="rounded-xl border border-warn/30 bg-warn/10 p-4">
      <div className="flex items-center gap-2 text-[13px] font-medium text-warn">
        <TriangleAlert className="size-4" />
        {title}
      </div>
      <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-center">
        <code className="min-w-0 flex-1 rounded-lg border border-line bg-fill px-3 py-2 font-mono text-xs break-all text-fg">
          {secret}
        </code>
        <div className="flex shrink-0 gap-2">
          <CopyButton value={secret} />
          {onDismiss && (
            <Button size="sm" variant="ghost" onClick={onDismiss}>
              Done
            </Button>
          )}
        </div>
      </div>
      {children}
    </div>
  );
}

const noop = () => () => {};

/** This site's origin, without a hydration mismatch. */
export function useOrigin(): string {
  return useSyncExternalStore(
    noop,
    () => window.location.origin,
    () => ""
  );
}
