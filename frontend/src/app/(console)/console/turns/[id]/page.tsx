"use client";

// One turnover: its steps in order, what each waits on, and the actions that
// move it forward. A step can open a work order, which completes the step
// when it's resolved. Steps that need a photo keep them on the step.

import { useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Ban,
  Camera,
  Check,
  CircleCheckBig,
  Play,
  RotateCcw,
  SkipForward,
  TriangleAlert,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { stepTone, turns, type Step, type Turn } from "@/lib/turns";
import { useAuth } from "@/lib/auth";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { Documents } from "../../flips/_parts/Documents";

const humanize = (k: string) => k.charAt(0).toUpperCase() + k.slice(1);

export default function TurnPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const qc = useQueryClient();
  const turn = useQuery({
    queryKey: ["turns", "one", id],
    queryFn: () => turns.get(id),
    enabled: can("maintenance:read"),
  });
  const [busy, setBusy] = useState(false);
  const [ask, setAsk] = useState<null | {
    title: string;
    description: string;
    action: string;
    onDone: (reason: string) => void;
  }>(null);

  async function run(fn: () => Promise<Turn>, ok?: string) {
    setBusy(true);
    try {
      const t = await fn();
      qc.setQueryData(["turns", "one", id], t);
      void qc.invalidateQueries({ queryKey: ["turns"], exact: false });
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  const back = (
    <Link
      href="/console/turns"
      className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
    >
      <ArrowLeft className="size-4" />
      Turnovers
    </Link>
  );

  if (turn.error)
    return (
      <div className="space-y-6">
        {back}
        <Panel>
          <EmptyState
            icon={<TriangleAlert />}
            title="Couldn't load this turnover"
            description={turn.error.message}
          />
        </Panel>
      </div>
    );
  const t = turn.data;
  if (!t)
    return (
      <div className="space-y-6">
        {back}
        <Skeleton className="h-24 rounded-2xl" />
        <Skeleton className="h-80 rounded-2xl" />
      </div>
    );

  const active = t.status === "active";
  const finish = () => {
    if (t.unmet_required.length) {
      setAsk({
        title: "Finish with steps open?",
        description: `These required steps aren't done: ${t.unmet_required.join(", ")}. Say why you're finishing anyway.`,
        action: "Finish turn",
        onDone: (why) =>
          run(() => turns.finish(t.id, why), "Turnover finished"),
      });
    } else void run(() => turns.finish(t.id), "Turnover finished");
  };

  return (
    <div className="space-y-6">
      {back}
      <PageHeader
        eyebrow={
          <span className="inline-flex items-center gap-2">
            Turnover
            <Badge
              tone={
                t.status === "done"
                  ? "good"
                  : t.overdue
                    ? "bad"
                    : t.status === "cancelled"
                      ? "neutral"
                      : "info"
              }
            >
              {t.overdue
                ? "past target"
                : t.status === "active"
                  ? "in progress"
                  : t.status}
            </Badge>
          </span>
        }
        title={`${t.property_name} · Unit ${t.unit_number ?? "—"}`}
        description={
          <>
            Started {t.started_on}
            {t.target_date ? ` · target ${t.target_date}` : ""}
            {t.finished_on ? ` · finished ${t.finished_on}` : ""}
          </>
        }
        actions={
          manage &&
          active && (
            <>
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  if (confirm("Cancel this turnover?"))
                    void run(() => turns.cancel(t.id), "Cancelled");
                }}
              >
                <Ban />
                Cancel turn
              </Button>
              <Button disabled={busy} onClick={finish}>
                <CircleCheckBig />
                Finish turn
              </Button>
            </>
          )
        }
      />

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Stat label="Steps done" value={`${t.done} of ${t.total}`} />
        <Stat label="Days vacant" value={t.days_open} />
        <Stat label="Cost to turn" value={t.cost_label} />
        <Stat
          label="Required left"
          value={t.unmet_required.length}
          tone={t.unmet_required.length ? "warn" : "good"}
        />
      </section>

      {t.override_reason && (
        <Panel className="flex items-start gap-3 border-warn/30 p-4 text-[13px] text-warn">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          Finished with required steps open: {t.override_reason}
        </Panel>
      )}

      <Panel>
        <PanelHeader
          title="Steps"
          description="A step opens once the steps it waits on are done."
        />
        <ol className="mt-3 divide-y divide-line">
          {(t.steps ?? []).map((s, i) => (
            <StepRow
              key={s.id}
              n={i + 1}
              step={s}
              can={manage && active}
              busy={busy}
              run={run}
              askSkip={(onDone) =>
                setAsk({
                  title: `Skip “${s.title}”?`,
                  description: "Say why. It's kept on the turnover.",
                  action: "Skip step",
                  onDone,
                })
              }
            />
          ))}
        </ol>
        {(t.steps ?? []).length === 0 && (
          <EmptyState title="No steps on this turnover" className="py-8" />
        )}
      </Panel>

      <ReasonDialog ask={ask} onClose={() => setAsk(null)} />
    </div>
  );
}

function StepRow({
  n,
  step: s,
  can,
  busy,
  run,
  askSkip,
}: {
  n: number;
  step: Step;
  can: boolean;
  busy: boolean;
  run: (fn: () => Promise<Turn>, ok?: string) => Promise<void>;
  askSkip: (onDone: (reason: string) => void) => void;
}) {
  const [photos, setPhotos] = useState(false);
  const finished = s.status === "done" || s.status === "skipped";
  const act = (
    action: "start" | "complete" | "skip" | "reopen",
    extra: { reason?: string } = {}
  ) => run(() => turns.step(s.id, { action, ...extra }));

  return (
    <li className="px-5 py-3.5">
      <div className="flex flex-wrap items-center gap-3">
        <span
          className={cn(
            "flex size-7 shrink-0 items-center justify-center rounded-full border text-xs font-semibold",
            s.status === "done"
              ? "border-good/30 bg-good/12 text-good"
              : "border-line bg-fill text-fg-2"
          )}
        >
          {s.status === "done" ? <Check className="size-3.5" /> : n}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={cn(
                "text-[14px] font-medium",
                finished ? "text-fg-3 line-through" : "text-fg"
              )}
            >
              {s.title}
            </span>
            {!s.required && <Badge>optional</Badge>}
            {s.requires_photo && <Badge tone="info">photo</Badge>}
            {s.overdue && <Badge tone="bad">overdue</Badge>}
          </div>
          <div className="text-xs text-fg-3">
            {humanize(s.owner_role)}
            {s.due_on ? ` · due ${s.due_on}` : ""}
            {s.waiting_on.length > 0 &&
              ` · waiting on ${s.waiting_on.join(", ")}`}
            {s.skip_reason && ` · skipped: ${s.skip_reason}`}
          </div>
        </div>
        {s.ticket_id && (
          <Link
            href={`/console/maintenance/${s.ticket_id}`}
            className="inline-flex items-center gap-1 text-xs font-medium text-accent hover:underline"
          >
            <Wrench className="size-3.5" />
            Work order
            {s.ticket_status ? ` (${s.ticket_status.replace("_", " ")})` : ""}
          </Link>
        )}
        {s.cost_label && (
          <span className="figure text-[13px] text-fg-2">{s.cost_label}</span>
        )}
        <Badge tone={stepTone(s.status)}>{s.status}</Badge>
      </div>
      {(can || s.requires_photo || photos) && (
        <div className="mt-2 flex flex-wrap gap-1.5 pl-10">
          {can && s.status === "ready" && (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => act("start")}
            >
              <Play />
              Start
            </Button>
          )}
          {can && (s.status === "ready" || s.status === "doing") && (
            <Button size="sm" disabled={busy} onClick={() => act("complete")}>
              <Check />
              Done
            </Button>
          )}
          {can && !finished && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => askSkip((reason) => void act("skip", { reason }))}
            >
              <SkipForward />
              Skip
            </Button>
          )}
          {can && finished && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => act("reopen")}
            >
              <RotateCcw />
              Reopen
            </Button>
          )}
          {can && !finished && !s.ticket_id && (
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() =>
                run(() => turns.stepTicket(s.id), "Work order opened")
              }
            >
              <Wrench />
              Open work order
            </Button>
          )}
          {(s.requires_photo || photos) && (
            <Button
              size="sm"
              variant="ghost"
              aria-expanded={photos}
              onClick={() => setPhotos(!photos)}
            >
              <Camera />
              {photos ? "Hide photos" : "Photos"}
            </Button>
          )}
        </div>
      )}
      {photos && (
        <div className="mt-3 pl-10">
          <Documents
            bare
            ownerType="process_step"
            ownerId={s.id}
            title="Photos"
            accept="image/*"
          />
        </div>
      )}
    </li>
  );
}

/** Asks for a reason before an action that needs one. */
function ReasonDialog({
  ask,
  onClose,
}: {
  ask: null | {
    title: string;
    description: string;
    action: string;
    onDone: (reason: string) => void;
  };
  onClose: () => void;
}) {
  return (
    <Dialog open={!!ask} onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        {ask && <ReasonForm key={ask.title} ask={ask} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function ReasonForm({
  ask,
  onClose,
}: {
  ask: {
    title: string;
    description: string;
    action: string;
    onDone: (reason: string) => void;
  };
  onClose: () => void;
}) {
  const [reason, setReason] = useState("");
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (!reason.trim()) return;
        ask.onDone(reason.trim());
        onClose();
      }}
    >
      <DialogTitle className="text-[17px] font-semibold">
        {ask.title}
      </DialogTitle>
      <DialogDescription className="mt-1 text-[13px] text-fg-3">
        {ask.description}
      </DialogDescription>
      <textarea
        autoFocus
        rows={3}
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        aria-label="Reason"
        className="mt-4 w-full rounded-xl border border-line-strong bg-fill px-3.5 py-2.5 text-sm text-fg outline-none focus:border-accent"
      />
      <div className="mt-4 flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onClose}>
          Back
        </Button>
        <Button type="submit" disabled={!reason.trim()}>
          {ask.action}
        </Button>
      </div>
    </form>
  );
}
