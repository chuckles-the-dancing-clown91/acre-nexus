"use client";

// One turnover: its steps in order, what each waits on, and the actions that
// move it forward. Work orders open from a step and complete it when resolved.

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { toast } from "sonner";
import { turns, stepTone, type Step, type Turn } from "@/lib/turns";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";

const humanize = (k: string) => k.charAt(0).toUpperCase() + k.slice(1);

export default function TurnPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const [turn, setTurn] = useState<Turn | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    turns
      .get(id)
      .then(setTurn)
      .catch((e: Error) => setError(e.message));
  }, [id]);
  useEffect(load, [load]);

  const run = async (fn: () => Promise<Turn>, ok?: string) => {
    setBusy(true);
    try {
      setTurn(await fn());
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!turn) return <p className="text-sm text-ink-3">Loading…</p>;

  const active = turn.status === "active";
  const finish = () => {
    if (turn.unmet_required.length) {
      const why = window.prompt(
        `These required steps are still open:\n${turn.unmet_required.join(
          ", "
        )}\n\nWhy finish anyway?`
      );
      if (!why?.trim()) return;
      void run(() => turns.finish(turn.id, why.trim()), "Turnover finished");
    } else {
      void run(() => turns.finish(turn.id), "Turnover finished");
    }
  };

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-3">
        <div>
          <Link href="/console/turns" className="text-xs text-ink-3">
            ← Turnovers
          </Link>
          <h1 className="font-display text-2xl font-bold">
            {turn.property_name} · Unit {turn.unit_number ?? "—"}
          </h1>
          <p className="text-sm text-ink-3">
            Started {turn.started_on}
            {turn.target_date ? ` · target ${turn.target_date}` : ""}
            {turn.finished_on ? ` · finished ${turn.finished_on}` : ""}
          </p>
        </div>
        <Badge
          className="ml-2"
          tone={turn.status === "done" ? "good" : turn.overdue ? "bad" : "info"}
        >
          {turn.overdue ? "Past target" : turn.status}
        </Badge>
        {manage && active && (
          <div className="ml-auto flex gap-2">
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => {
                if (window.confirm("Cancel this turnover?"))
                  void run(() => turns.cancel(turn.id), "Cancelled");
              }}
            >
              Cancel
            </Button>
            <Button disabled={busy} onClick={finish}>
              Finish turn
            </Button>
          </div>
        )}
      </div>

      <div className="grid gap-3 sm:grid-cols-4">
        <StatTile label="Steps done" value={`${turn.done} of ${turn.total}`} />
        <StatTile label="Days vacant" value={String(turn.days_open)} />
        <StatTile label="Cost to turn" value={turn.cost_label} />
        <StatTile
          label="Required left"
          value={String(turn.unmet_required.length)}
        />
      </div>
      {turn.override_reason && (
        <p className="rounded-xl bg-warn-soft px-4 py-3 text-sm text-warn">
          Finished with required steps open: {turn.override_reason}
        </p>
      )}

      <Card className="divide-y divide-line">
        {(turn.steps ?? []).map((s, i) => (
          <StepRow
            key={s.id}
            n={i + 1}
            step={s}
            can={manage && active}
            busy={busy}
            run={run}
          />
        ))}
      </Card>
    </div>
  );
}

function StepRow({
  n,
  step: s,
  can,
  busy,
  run,
}: {
  n: number;
  step: Step;
  can: boolean;
  busy: boolean;
  run: (fn: () => Promise<Turn>, ok?: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const finished = s.status === "done" || s.status === "skipped";
  const act = (
    action: "start" | "complete" | "skip" | "reopen",
    extra: { reason?: string } = {}
  ) => run(() => turns.step(s.id, { action, ...extra }));

  return (
    <div className="px-5 py-4">
      <div className="flex flex-wrap items-center gap-3">
        <span className="flex h-7 w-7 items-center justify-center rounded-full bg-surface-2 text-xs font-bold text-ink-2">
          {n}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`font-semibold ${
                finished ? "text-ink-3 line-through" : ""
              }`}
            >
              {s.title}
            </span>
            {!s.required && <Badge>optional</Badge>}
            {s.requires_photo && <Badge tone="info">photo</Badge>}
            {s.overdue && <Badge tone="bad">overdue</Badge>}
          </div>
          <div className="text-xs text-ink-3">
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
            className="text-xs font-semibold text-accent-2"
          >
            Work order{s.ticket_status ? ` (${s.ticket_status})` : ""}
          </Link>
        )}
        {s.cost_label && <span className="text-sm">{s.cost_label}</span>}
        <Badge tone={stepTone(s.status)}>{s.status}</Badge>
        {can && (
          <div className="flex gap-1">
            {s.status === "ready" && (
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => act("start")}
              >
                Start
              </Button>
            )}
            {(s.status === "ready" || s.status === "doing") && (
              <Button disabled={busy} onClick={() => act("complete")}>
                Done
              </Button>
            )}
            {!finished && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  const reason = window.prompt("Why skip this step?");
                  if (reason?.trim())
                    void act("skip", { reason: reason.trim() });
                }}
              >
                Skip
              </Button>
            )}
            {finished && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => act("reopen")}
              >
                Reopen
              </Button>
            )}
            {!finished && !s.ticket_id && (
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() =>
                  run(() => turns.stepTicket(s.id), "Work order opened")
                }
              >
                Work order
              </Button>
            )}
          </div>
        )}
        {(s.requires_photo || open) && (
          <button
            onClick={() => setOpen(!open)}
            className="text-xs font-semibold text-ink-3"
          >
            {open ? "Hide photos" : "Photos"}
          </button>
        )}
      </div>
      {open && (
        <div className="mt-3">
          <DocumentsCard ownerType="process_step" ownerId={s.id} />
        </div>
      )}
    </div>
  );
}
