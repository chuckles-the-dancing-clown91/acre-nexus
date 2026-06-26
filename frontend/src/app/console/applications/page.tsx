"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { Application } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";

export default function ApplicationsPage() {
  const [apps, setApps] = useState<Application[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.applications().then(setApps).catch((e) => setError(e.message));
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Applications
        </h1>
        <p className="text-ink-3">
          Applicants submitted through your public website (screened
          automatically).
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="divide-y divide-line">
          {apps?.map((a) => (
            <div key={a.id} className="flex items-center gap-4 px-5 py-3.5">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{a.applicant_name}</div>
                <div className="truncate text-sm text-ink-3">{a.email}</div>
              </div>
              <div className="hidden text-sm text-ink-2 sm:block">
                {a.credit_score ? `Credit ${a.credit_score}` : "—"}
              </div>
              <div className="hidden text-sm text-ink-2 sm:block">
                {a.annual_income_label}/yr
              </div>
              <Badge tone={statusTone(a.status)}>{a.status}</Badge>
            </div>
          ))}
          {apps && apps.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No applications yet — submit one from the public website.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
