"use client";

// Getting set up: the steps that take a new workspace live. Each one is
// checked against real data on the server; Re-check asks it to look again.

import Link from "next/link";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, ChevronRight, RefreshCw, Rocket } from "lucide-react";
import { toast } from "sonner";
import { api, type OnboardingStep } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

/** How to act on each step. */
const ACTIONS: Record<string, { hint: string; href?: string; cta?: string }> = {
  firm_admin_accepted: {
    hint: "Done on its own once the company owner has signed in.",
  },
  branding_configured: {
    hint: "Your name, logo and colours, so the website and portals look like you.",
    href: "/console/branding",
    cta: "Set branding",
  },
  domains_configured: {
    hint: "Connect your own domain, or use the subdomain reserved for you.",
    href: "/console/domains",
    cta: "Add a domain",
  },
  entities_created: {
    hint: "The LLCs that hold title to your properties.",
    href: "/console/llcs",
    cta: "Add an LLC",
  },
  banking_linked: {
    hint: "An operating account and a trust account for each LLC.",
    href: "/console/llcs",
    cta: "Open LLCs",
  },
  portfolio_imported: {
    hint: "Add your first property, tied to an LLC so the books are right from day one.",
    href: "/console/properties/onboard",
    cta: "Add a property",
  },
  staff_invited: {
    hint: "Invite your team and give each person a role.",
    href: "/console/members",
    cta: "Invite people",
  },
};

const KEY = ["onboarding-workflow"];

export default function OnboardingPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const allowed = can("tenant:manage");
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: KEY,
    queryFn: api.onboardingWorkflow,
    enabled: scoped && allowed,
  });
  const recheck = useMutation({
    mutationFn: api.advanceOnboarding,
    onSuccess: (snap) => {
      qc.setQueryData(KEY, snap);
      toast.success(snap.live ? "Your workspace is live" : "Checked again");
    },
    onError: (e) => toast.error(e.message || "Couldn't check again"),
  });

  const snap = q.data;
  const required = snap?.steps.filter((s) => !s.optional) ?? [];
  const done = required.filter((s) => s.complete).length;
  const pct = required.length ? Math.round((done / required.length) * 100) : 0;
  const next = snap?.steps.find((s) => !s.complete && !s.optional);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Workspace"
        title="Getting set up"
        description="Finish these to take your workspace live. Each is checked against your real data."
        actions={
          allowed && (
            <>
              {snap && (
                <Badge tone={snap.live ? "good" : "info"} dot>
                  {snap.live ? "Live" : snap.state.replace(/_/g, " ")}
                </Badge>
              )}
              <Button
                variant="secondary"
                loading={recheck.isPending}
                disabled={!snap}
                onClick={() => recheck.mutate()}
              >
                {!recheck.isPending && <RefreshCw />}
                Re-check
              </Button>
            </>
          )
        }
      />

      {!allowed && (
        <Panel>
          <EmptyState
            icon={<Rocket />}
            title="Setup is for company owners"
            description="Ask a company owner to finish setting up the workspace."
          />
        </Panel>
      )}

      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}
      {q.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load setup: {q.error.message}
        </Panel>
      )}

      {snap && (
        <>
          <Panel className="p-5">
            {snap.live ? (
              <div className="flex items-center gap-3">
                <span className="flex size-10 items-center justify-center rounded-xl border border-good/30 bg-good/10 text-good">
                  <Rocket className="size-5" />
                </span>
                <div>
                  <div className="text-[15px] font-semibold text-fg">
                    Your workspace is live
                  </div>
                  <p className="text-[13px] text-fg-3">
                    Every required step is done. Optional ones are below.
                  </p>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="text-[14px] font-medium text-fg">
                    {done} of {required.length} required steps done
                  </span>
                  <span className="figure text-[13px] text-fg-3">{pct}%</span>
                </div>
                <div className="h-2 w-full overflow-hidden rounded-full bg-fill-2">
                  <div
                    className="h-full rounded-full bg-accent transition-[width] duration-500"
                    style={{ width: `${pct}%` }}
                  />
                </div>
                {next && (
                  <p className="text-[13px] text-fg-3">
                    Up next:{" "}
                    <span className="font-medium text-fg">{next.label}</span>
                  </p>
                )}
              </div>
            )}
          </Panel>

          <Panel>
            <PanelHeader
              title="Steps"
              action={
                <Badge tone="neutral">
                  {snap.steps.filter((s) => s.complete).length} /{" "}
                  {snap.steps.length}
                </Badge>
              }
            />
            <ol className="mt-3 divide-y divide-line">
              {snap.steps.map((s, i) => (
                <StepRow
                  key={s.key}
                  step={s}
                  index={i + 1}
                  isNext={s.key === next?.key}
                />
              ))}
            </ol>
          </Panel>
        </>
      )}
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
    <li
      className={cn(
        "flex flex-col gap-3 px-5 py-3.5 sm:flex-row sm:items-center",
        isNext && "bg-accent/5"
      )}
    >
      <div className="flex min-w-0 flex-1 items-start gap-3">
        <span
          aria-hidden
          className={cn(
            "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full border text-[13px] font-medium",
            step.complete
              ? "border-good/30 bg-good/10 text-good"
              : isNext
                ? "border-accent/40 bg-accent/10 text-accent"
                : "border-line text-fg-3"
          )}
        >
          {step.complete ? <Check className="size-4" /> : index}
        </span>
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[14px] font-medium text-fg">
              {step.label}
            </span>
            {step.optional && <Badge tone="neutral">Optional</Badge>}
            <Badge tone={step.complete ? "good" : isNext ? "accent" : "warn"}>
              {step.complete ? "Done" : isNext ? "Next" : "To do"}
            </Badge>
          </div>
          {action.hint && (
            <p className="mt-0.5 text-[13px] text-fg-3">{action.hint}</p>
          )}
        </div>
      </div>
      {action.href && action.cta && (
        <Button
          asChild
          size="sm"
          variant={step.complete ? "ghost" : isNext ? "primary" : "secondary"}
          className="self-start sm:self-auto"
        >
          <Link href={action.href}>
            {step.complete ? "Review" : action.cta}
            <ChevronRight />
          </Link>
        </Button>
      )}
    </li>
  );
}
