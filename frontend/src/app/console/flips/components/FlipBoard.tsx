"use client";

// Flip deal board (kanban-style columns over the pipeline stages). This is the
// heavy, module-specific component that the Flips page loads lazily via
// `next/dynamic`, so its code only ships to tenants who have the preview module
// enabled and actually open the page.

import { useEffect, useState } from "react";
import { api, type FlipPipeline } from "@/lib/api";
import { Card } from "@/components/ui";

export default function FlipBoard() {
  const [data, setData] = useState<FlipPipeline | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.flipPipeline().then(setData).catch((e) => setError(e.message));
  }, []);

  if (error) {
    return (
      <div className="rounded-xl border border-bad-soft bg-bad-soft/40 px-4 py-3 text-sm text-bad">
        {error}
      </div>
    );
  }
  if (!data) return <div className="text-ink-3">Loading pipeline…</div>;

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-5">
      {data.stages.map((stage) => (
        <div key={stage.key} className="space-y-2">
          <div className="flex items-center justify-between px-1">
            <h3 className="font-display text-sm font-bold">{stage.label}</h3>
            <span className="text-xs text-ink-3">0</span>
          </div>
          <Card className="flex min-h-28 items-center justify-center p-3 text-center text-xs text-ink-3">
            No deals yet
          </Card>
        </div>
      ))}
    </div>
  );
}
