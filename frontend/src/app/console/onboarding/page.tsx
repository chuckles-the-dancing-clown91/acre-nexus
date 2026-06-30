"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, type OnboardingSnapshot, type OnboardingStep } from "@/lib/api";
import { Badge, Card } from "@/components/ui";

/** UI metadata per step key: how to act on it. */
const ACTIONS: Record<string, { hint: string; href?: string; cta?: string }> = {
  firm_admin_accepted: {
    hint: "Completed automatically once the firm owner has signed in.",
  },
  branding_configured: {
    hint: "Set your company name, logo, and brand colors so the white-label site and portals match your firm.",
    href: "/console/branding",
    cta: "Set branding",
  },
  domains_configured: {
    hint: "Connect a custom domain (or use your reserved subdomain) for the admin app and portals.",
    href: "/console/domains",
    cta: "Manage domains",
  },
  entities_created: {
    hint: "Create the LLCs (legal entities) that hold title to your properties.",
    href: "/console/llcs",
    cta: "Add an LLC",
  },
  banking_linked: {
    hint: "Add an operating and a trust (escrow) account to each legal entity.",
    href: "/console/llcs",
    cta: "Open entities",
  },
  portfolio_imported: {
    hint: "Onboard your first property — bound to a legal entity so the books are correct from day one.",
    href: "/console/properties/onboard",
    cta: "Onboard a property",
  },
  staff_invited: {
    hint: "Invite your team and assign them scoped roles.",
    href: "/console/members",
    cta: "Invite staff",
  },
};

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

  const required = snap?.steps.filter((s) => !s.optional) ?? [];
  const requiredDone = required.filter((s) => s.complete).length;
  const pct = required.length
    ? Math.round((requiredDone / required.length) * 100)
    : 0;
  // The first incomplete step is the one to nudge.
  const nextStep = snap?.steps.find((s) => !s.complete && !s.optional);

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

      {/* Progress */}
      {snap && (
        <Card className="space-y-3 p-5">
          {snap.live ? (
            <div className="font-display text-lg font-bold text-good">
              🎉 Your workspace is live — all required steps are complete.
            </div>
          ) : (
            <>
              <div className="flex items-center justify-between text-sm">
                <span className="font-semibold">
                  {requiredDone} of {required.length} required steps complete
                </span>
                <span className="text-ink-3">{pct}%</span>
              </div>
              <div className="h-2 w-full overflow-hidden rounded-full bg-surface-2">
                <div
                  className="h-full rounded-full bg-accent transition-all"
                  style={{ width: `${pct}%` }}
                />
              </div>
              {nextStep && (
                <p className="text-sm text-ink-3">
                  Up next: <b>{nextStep.label}</b>
                </p>
              )}
            </>
          )}
        </Card>
      )}

      {/* Steps */}
      <ol className="space-y-3">
        {snap?.steps.map((s, i) => (
          <StepRow
            key={s.key}
            step={s}
            index={i + 1}
            isNext={!s.complete && s.key === nextStep?.key}
          />
        ))}
      </ol>
    </div>
  );
}

function StepRow({
  step,
  index,
  isNext,
}: {
  step: OnboardingStep;
  index: number;
  isNext: boolean;
}) {
  const action = ACTIONS[step.key] ?? { hint: "" };
  return (
    <li>
      <Card
        className={`flex flex-wrap items-center gap-4 p-4 ${
          isNext ? "ring-2 ring-accent" : ""
        }`}
      >
        <span
          className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-bold ${
            step.complete
              ? "bg-good/15 text-good"
              : "border border-line text-ink-3"
          }`}
          aria-hidden
        >
          {step.complete ? "✓" : index}
        </span>
        <div className="min-w-[180px] flex-1">
          <div className="flex items-center gap-2">
            <span className="font-semibold">{step.label}</span>
            {step.optional && <Badge tone="neutral">optional</Badge>}
          </div>
          <p className="text-sm text-ink-3">{action.hint}</p>
        </div>
        <Badge tone={step.complete ? "good" : isNext ? "info" : "warn"}>
          {step.complete ? "done" : isNext ? "next" : "to do"}
        </Badge>
        {action.href && action.cta && (
          <Link
            href={action.href}
            className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
              step.complete
                ? "border border-line text-ink-2"
                : "bg-accent text-white"
            }`}
          >
            {step.complete ? "Review" : action.cta}
          </Link>
        )}
      </Card>
    </li>
  );
}
