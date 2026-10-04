"use client";

// Small pieces shared by the leasing pages: the no-access panel, a copy-link
// button for one-time signing links, status tones, and money parsing.

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Check, Copy, Lock } from "lucide-react";
import type { Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export function NoAccess({ what, perm }: { what: string; perm: string }) {
  return (
    <Panel className="mx-auto mt-10 max-w-lg">
      <EmptyState
        icon={<Lock />}
        title={`You can't see ${what}`}
        description={
          <>
            Ask an admin for the <span className="font-mono">{perm}</span>{" "}
            permission.
          </>
        }
      />
    </Panel>
  );
}

/** Copies a URL to the clipboard and confirms for a moment. */
export function CopyLink({
  url,
  label = "Copy link",
}: {
  url: string;
  label?: string;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      size="sm"
      variant="secondary"
      type="button"
      onClick={() => {
        void navigator.clipboard?.writeText(url);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      }}
    >
      {copied ? <Check /> : <Copy />}
      {copied ? "Copied" : label}
    </Button>
  );
}

export function signerTone(status: string): Tone {
  switch (status) {
    case "signed":
      return "good";
    case "viewed":
      return "info";
    case "declined":
      return "bad";
    case "sent":
      return "warn";
    default:
      return "neutral";
  }
}

export function envelopeTone(status: string): Tone {
  switch (status) {
    case "completed":
      return "good";
    case "partially_signed":
      return "info";
    case "sent":
      return "warn";
    case "declined":
      return "bad";
    default:
      return "neutral";
  }
}

export function paymentTone(status: string): Tone {
  if (status === "current" || status === "paid") return "good";
  if (status === "partial") return "warn";
  if (status === "late") return "bad";
  return "neutral";
}

/** "12.50" → 1250, or null when it isn't a number. */
export function toCents(dollars: string): number | null {
  const n = parseFloat(dollars);
  return Number.isFinite(n) ? Math.round(n * 100) : null;
}

export function errorText(e: unknown, fallback = "That didn't work"): string {
  return e instanceof Error ? e.message : fallback;
}

export function humanize(key: string): string {
  return key.charAt(0).toUpperCase() + key.slice(1).replace(/_/g, " ");
}

/** A small uppercase table header cell, matching the property units table. */
export function Th({
  children,
  num,
}: {
  children: React.ReactNode;
  num?: boolean;
}) {
  return (
    <th
      scope="col"
      className={
        "px-4 py-2.5 text-[11px] font-medium tracking-wide whitespace-nowrap text-fg-3 uppercase" +
        (num ? " text-right" : "")
      }
    >
      {children}
    </th>
  );
}

/** A figure with an icon tile, as on the service desk. */
export function IconStat({
  icon,
  label,
  value,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  tone?: "bad" | "warn";
}) {
  return (
    <Panel className="flex items-center gap-4 p-4">
      <span
        className={cn(
          "flex size-10 items-center justify-center rounded-xl border [&_svg]:size-[18px]",
          tone === "bad"
            ? "border-bad/30 bg-bad/10 text-bad"
            : tone === "warn"
              ? "border-warn/30 bg-warn/10 text-warn"
              : "border-line bg-fill text-fg-2"
        )}
      >
        {icon}
      </span>
      <div className="min-w-0">
        <div className="figure truncate text-[24px] leading-none font-semibold text-fg">
          {value}
        </div>
        <div className="mt-1 text-xs text-fg-3 first-letter:uppercase">
          {label}
        </div>
      </div>
    </Panel>
  );
}

/** Classes for a plain input, select or textarea in a form. */
export const inputClass =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

/** A labelled form control. */
export function F({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={cn("block", className)}>
      <span className="mb-1 block text-xs font-medium text-fg-3">{label}</span>
      {children}
    </label>
  );
}

/**
 * Runs one action at a time with a busy key, toasts what went wrong, and
 * refetches the given query keys afterwards.
 */
export function useRun(refresh: readonly (readonly unknown[])[]) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState<string | null>(null);
  async function run(
    key: string,
    fn: () => Promise<unknown>,
    ok?: string
  ): Promise<boolean> {
    setBusy(key);
    try {
      await fn();
      if (ok) toast.success(ok);
      return true;
    } catch (e) {
      toast.error(errorText(e));
      return false;
    } finally {
      setBusy(null);
      for (const queryKey of refresh) void qc.invalidateQueries({ queryKey });
    }
  }
  return { busy, run };
}
