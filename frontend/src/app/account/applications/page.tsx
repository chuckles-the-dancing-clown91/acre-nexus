"use client";

// "My applications" — the renter-facing view of the leasing pipeline: every
// application the signed-in user submitted (through their profile, or earlier
// through the public site with the same email), with live pipeline status.

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Application } from "@/lib/types";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

const PIPELINE = ["Screening", "Approved", "Leased"];

export default function MyApplicationsPage() {
  const { user, loading } = useAuth();
  const [apps, setApps] = useState<Application[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user) return;
    api
      .myApplications()
      .then((a) => {
        setApps(a);
        setError(null);
      })
      .catch((e) => setError(e.message));
  }, [user]);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[860px] px-6 py-8">
        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
          My applications
        </h1>
        <p className="mb-6 text-ink-3">
          Track every application you&apos;ve submitted — screening runs
          automatically and you&apos;ll be emailed at each step.
        </p>

        {!loading && !user && (
          <Card className="p-8 text-center">
            <p className="mb-3 text-ink-2">Sign in to see your applications.</p>
            <Link
              href="/login"
              className="inline-block rounded-xl bg-accent px-5 py-2.5 text-sm font-bold text-on-accent"
            >
              Sign in
            </Link>
          </Card>
        )}

        {error && <p className="text-bad">{error}</p>}

        {user && apps && apps.length === 0 && (
          <Card className="p-8 text-center text-ink-3">
            No applications yet —{" "}
            <Link href="/" className="underline">
              browse listings
            </Link>{" "}
            and apply.
          </Card>
        )}

        <div className="space-y-4">
          {apps?.map((a) => {
            const stageIdx = PIPELINE.indexOf(a.status);
            return (
              <Card key={a.id} className="p-5">
                <div className="mb-3 flex flex-wrap items-center gap-3">
                  <span className="font-semibold">
                    Application from {a.created_at.slice(0, 10)}
                  </span>
                  {a.screening_status && (
                    <Badge
                      tone={a.screening_status === "cleared" ? "good" : "bad"}
                    >
                      screening {a.screening_status}
                    </Badge>
                  )}
                  <Badge tone={statusTone(a.status)}>{a.status}</Badge>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {PIPELINE.map((stage, i) => {
                    const reached =
                      a.status === stage || (stageIdx >= 0 && i <= stageIdx);
                    const offRamp =
                      a.status === "Declined" || a.status === "Withdrawn";
                    return (
                      <div key={stage} className="flex items-center gap-2">
                        <span
                          className={
                            "flex h-6 items-center rounded-full px-2.5 text-xs font-semibold " +
                            (a.status === stage
                              ? "bg-accent text-on-accent"
                              : reached && !offRamp
                                ? "bg-good-soft text-good"
                                : "bg-surface-2 text-ink-3")
                          }
                        >
                          {stage}
                        </span>
                        {i < PIPELINE.length - 1 && (
                          <span className="h-px w-4 bg-line" />
                        )}
                      </div>
                    );
                  })}
                  {(a.status === "Declined" || a.status === "Withdrawn") && (
                    <Badge tone={a.status === "Declined" ? "bad" : "neutral"}>
                      {a.status}
                    </Badge>
                  )}
                </div>
                <p className="mt-3 text-sm text-ink-3">
                  {a.status === "Screening" &&
                    "Background screening is in progress — we'll email you as soon as there's a decision."}
                  {a.status === "Approved" &&
                    "Approved! The leasing team will reach out with your lease to sign electronically."}
                  {a.status === "Leased" &&
                    "Your lease has been created — check your email for the signing link."}
                  {a.status === "Declined" &&
                    "We couldn't move forward with this application."}
                  {a.status === "Withdrawn" &&
                    "This application was withdrawn."}
                </p>
              </Card>
            );
          })}
        </div>
      </main>
    </>
  );
}
