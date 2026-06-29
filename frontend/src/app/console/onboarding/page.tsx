"use client";

import { useEffect, useState } from "react";
import { api, type OnboardingSnapshot } from "@/lib/api";
import { Badge, Card } from "@/components/ui";

export default function OnboardingPage() {
  const [snap, setSnap] = useState<OnboardingSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () =>
    api
      .onboardingWorkflow()
      .then(setSnap)
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
  }, []);

  async function refresh() {
    setBusy(true);
    setError(null);
    try {
      setSnap(await api.advanceOnboarding());
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const done = snap?.steps.filter((s) => s.complete).length ?? 0;
  const total = snap?.steps.length ?? 0;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex-1">
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Getting set up
          </h1>
          <p className="text-ink-3">
            Finish these steps to take your workspace live. Each is verified
            against your live data.
          </p>
        </div>
        {snap && (
          <Badge tone={snap.live ? "good" : "info"}>
            {snap.live ? "Live" : `State: ${snap.state}`}
          </Badge>
        )}
        <button
          onClick={refresh}
          disabled={busy}
          className="rounded-lg border border-line px-3 py-1.5 text-sm font-semibold disabled:opacity-50"
        >
          Re-check
        </button>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {snap && (
        <p className="text-sm text-ink-3">
          {done} of {total} steps complete
        </p>
      )}

      <div className="space-y-3">
        {snap?.steps.map((s) => (
          <Card key={s.key} className="flex items-center gap-4 p-4">
            <span
              className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-sm font-bold ${
                s.complete
                  ? "bg-good/15 text-good"
                  : "border border-line text-ink-3"
              }`}
              aria-hidden
            >
              {s.complete ? "✓" : "•"}
            </span>
            <div className="flex-1">
              <div className="font-semibold">{s.label}</div>
              <div className="font-mono text-xs text-ink-3">{s.key}</div>
            </div>
            {s.optional && <Badge tone="neutral">optional</Badge>}
            <Badge tone={s.complete ? "good" : "warn"}>
              {s.complete ? "done" : "to do"}
            </Badge>
          </Card>
        ))}
      </div>
    </div>
  );
}
