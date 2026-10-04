"use client";

// Time off requests: pending first (soonest first), then everything decided
// (newest first). Managers approve or deny with an optional note.

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { CalendarOff, Check, X } from "lucide-react";
import { toast } from "sonner";
import { team, type TimeOff } from "@/lib/backoffice";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass } from "@/components/ui/input";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const TONE: Record<TimeOff["status"], Tone> = {
  pending: "warn",
  approved: "good",
  denied: "bad",
  cancelled: "neutral",
};

const ORDER: Record<TimeOff["status"], number> = {
  pending: 0,
  approved: 1,
  denied: 2,
  cancelled: 3,
};

function fmtDate(ymd: string) {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function TimeOffList({
  list,
  loading,
  error,
  manage,
}: {
  list: TimeOff[] | undefined;
  loading: boolean;
  error: Error | null;
  manage: boolean;
}) {
  const sorted = [...(list ?? [])].sort(
    (a, b) =>
      ORDER[a.status] - ORDER[b.status] ||
      (a.status === "pending"
        ? a.starts_on.localeCompare(b.starts_on)
        : b.starts_on.localeCompare(a.starts_on))
  );

  return (
    <Panel className="overflow-hidden">
      {loading && (
        <div className="space-y-2 p-3">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="h-16" />
          ))}
        </div>
      )}
      {error && (
        <p className="p-4 text-[13px] text-bad">
          Couldn&apos;t load time off: {error.message}
        </p>
      )}
      {list && list.length === 0 && (
        <EmptyState
          icon={<CalendarOff />}
          title="No time off requests"
          description="Staff ask from My time; requests land here."
        />
      )}
      <ul className="divide-y divide-line">
        {sorted.map((r) => (
          <Row key={r.id} r={r} manage={manage} />
        ))}
      </ul>
    </Panel>
  );
}

function Row({ r, manage }: { r: TimeOff; manage: boolean }) {
  const qc = useQueryClient();
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  async function review(approve: boolean) {
    setBusy(true);
    try {
      await team.reviewTimeOff(r.id, approve, note.trim() || undefined);
      toast.success(
        approve
          ? `Approved ${r.user_name}'s time off`
          : `Denied ${r.user_name}'s time off`
      );
      await qc.invalidateQueries({ queryKey: ["team"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    } finally {
      setBusy(false);
    }
  }

  return (
    <li className="flex flex-col gap-3 px-5 py-4 lg:flex-row lg:items-center">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[14px] font-medium text-fg">{r.user_name}</span>
          <Badge tone={TONE[r.status]}>{r.status}</Badge>
        </div>
        <div className="text-[13px] text-fg-2">
          {fmtDate(r.starts_on)}
          {r.ends_on !== r.starts_on && <> to {fmtDate(r.ends_on)}</>} ·{" "}
          {r.days} day{r.days === 1 ? "" : "s"} · {r.kind}
        </div>
        {r.reason && (
          <div className="text-[13px] text-fg-3">&ldquo;{r.reason}&rdquo;</div>
        )}
        {r.review_note && (
          <div className="text-xs text-fg-3">Note: {r.review_note}</div>
        )}
      </div>
      {manage && r.status === "pending" && (
        <div className="flex flex-wrap items-center gap-2">
          <input
            aria-label={`Note for ${r.user_name}`}
            className={cn(fieldClass, "min-w-0 flex-1 sm:w-48 sm:flex-none")}
            placeholder="Note (optional)"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <Button size="sm" disabled={busy} onClick={() => review(true)}>
            <Check />
            Approve
          </Button>
          <Button
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={() => review(false)}
          >
            <X />
            Deny
          </Button>
        </div>
      )}
    </li>
  );
}
