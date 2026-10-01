"use client";

// Settings → Schedule: every job that runs on its own, when it last ran, what
// it did, and when it runs next. Run one now; see recent runs and failures.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { toast } from "sonner";
import { jobs, type Job } from "@/lib/jobs";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

function when(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function relative(iso: string) {
  const ms = new Date(iso).getTime() - Date.now();
  const mins = Math.round(ms / 60000);
  if (Math.abs(mins) < 1) return "now";
  const abs = Math.abs(mins);
  const text =
    abs < 60
      ? `${abs} min`
      : abs < 48 * 60
        ? `${Math.round(abs / 60)} h`
        : `${Math.round(abs / 1440)} days`;
  return mins > 0 ? `in ${text}` : `${text} ago`;
}

function tone(status: string): "good" | "warn" | "bad" | "neutral" | "info" {
  if (status === "completed") return "good";
  if (status === "failed") return "bad";
  if (status === "running") return "info";
  return "neutral";
}

/** A job's last result as short "key: value" pairs. */
function resultText(r: Record<string, unknown> | null): string {
  if (!r) return "";
  return Object.entries(r)
    .filter(([, v]) => typeof v !== "object" || v === null)
    .slice(0, 6)
    .map(([k, v]) => `${k.replace(/_/g, " ")}: ${String(v)}`)
    .join(" · ");
}

export default function SchedulePage() {
  const { can } = useAuth();
  const allowed = can("tenant:manage");
  const [rows, setRows] = useState<Job[] | null>(null);
  const [failed, setFailed] = useState<Job[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    Promise.all([jobs.schedule(), jobs.list({ status: "failed" })])
      .then(([s, f]) => {
        setRows(s);
        setFailed(f);
      })
      .catch((e: Error) => setError(e.message));
  }, []);
  useEffect(() => {
    if (allowed) load();
  }, [allowed, load]);

  if (!allowed)
    return (
      <Card className="p-6 text-ink-2">
        You need the <span className="font-mono">tenant:manage</span>{" "}
        permission.
      </Card>
    );
  if (error) return <p className="text-sm text-bad">{error}</p>;
  if (!rows) return <p className="text-sm text-ink-3">Loading…</p>;

  const runNow = async (j: Job) => {
    setBusy(j.id);
    try {
      await jobs.runNow(j.id);
      toast.success("It will run within a few seconds");
      setTimeout(load, 4000);
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="max-w-4xl space-y-5">
      <div>
        <Link href="/console/settings" className="text-xs text-ink-3">
          ← Settings
        </Link>
        <h1 className="font-display text-2xl font-bold">Schedule</h1>
        <p className="text-sm text-ink-3">
          The jobs that run on their own for this workspace. What each one sends
          is set in Settings under Reminders.
        </p>
      </div>

      <Card className="divide-y divide-line">
        {rows.map((j) => (
          <div
            key={j.id}
            className="flex flex-wrap items-start gap-3 px-5 py-4"
          >
            <div className="min-w-0 flex-1">
              <div className="font-semibold">{j.label ?? j.kind}</div>
              <div className="font-mono text-xs text-ink-3">{j.kind}</div>
              <div className="mt-1 text-xs text-ink-2">
                Last ran {when(j.updated_at)} · next {relative(j.run_at)}
              </div>
              {j.result && (
                <div className="mt-1 text-xs text-ink-3">
                  {resultText(j.result)}
                </div>
              )}
              {j.last_error && (
                <div className="mt-1 text-xs text-bad">{j.last_error}</div>
              )}
            </div>
            <Badge tone={tone(j.status)}>{j.status}</Badge>
            {j.status === "pending" && (
              <Button
                variant="outline"
                disabled={busy === j.id}
                onClick={() => runNow(j)}
              >
                Run now
              </Button>
            )}
          </div>
        ))}
        {rows.length === 0 && (
          <p className="px-5 py-8 text-center text-sm text-ink-3">
            No jobs are scheduled for this workspace yet. They start when the
            server starts.
          </p>
        )}
      </Card>

      <div>
        <h2 className="mb-2 font-display text-lg font-bold">Recent failures</h2>
        <Card className="divide-y divide-line">
          {failed.map((j) => (
            <div key={j.id} className="px-5 py-3 text-sm">
              <div className="flex gap-2">
                <span className="font-mono text-xs">{j.kind}</span>
                <span className="text-ink-3">{when(j.updated_at)}</span>
              </div>
              <div className="text-xs text-bad">{j.last_error}</div>
            </div>
          ))}
          {failed.length === 0 && (
            <p className="px-5 py-6 text-center text-sm text-ink-3">
              Nothing has failed.
            </p>
          )}
        </Card>
      </div>
    </div>
  );
}
