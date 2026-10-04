"use client";

// Related parties: deals between the family's own people and entities. Each
// needs a note on what the same thing costs at market and a decision by
// someone who isn't a party. Bills from a related vendor land here on their
// own and can't be approved until this is.

import { Suspense, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Scale } from "lucide-react";
import { toast } from "sonner";
import { family, type Review } from "@/lib/family";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { dollarsToCents } from "@/lib/showings";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { F, input, why } from "@/components/property/bits";

const TABS = [
  ["open", "Waiting"],
  ["approved", "Approved"],
  ["rejected", "Rejected"],
] as const;
type Tab = (typeof TABS)[number][0];

const SUBJECT = {
  vendor_bill: "Bill",
  lease: "Lease",
  deal: "Deal",
  other: "Other",
} as const;

export default function RelatedPartyPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Reviews />
    </Suspense>
  );
}

function Reviews() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const params = useSearchParams();
  const router = useRouter();
  const tab = (TABS.find(([k]) => k === params.get("tab"))?.[0] ??
    "open") as Tab;
  const q = useQuery({
    queryKey: ["related-party", tab],
    queryFn: () => family.reviews(tab),
    enabled: scoped && can("payable:read"),
  });
  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Money"
        title="Related parties"
        description="Deals between the family's own people and companies. Each needs a market-rate note and a yes from someone who isn't part of it."
      />
      <Tabs
        tabs={TABS}
        value={tab}
        onChange={(k) =>
          router.replace(`/console/related-party?tab=${k}`, { scroll: false })
        }
      />
      {q.isLoading && <Skeleton className="h-40 rounded-2xl" />}
      {q.data?.length === 0 && (
        <EmptyState
          icon={<Scale />}
          title={tab === "open" ? "Nothing waiting" : "None yet"}
          description="Mark a vendor as one of the family's own companies or people on its page, and its bills show up here."
        />
      )}
      <div className="space-y-3">
        {q.data?.map((r) => (
          <ReviewCard key={r.id} r={r} />
        ))}
      </div>
    </div>
  );
}

function ReviewCard({ r }: { r: Review }) {
  const { can } = useAuth();
  const qc = useQueryClient();
  const [market, setMarket] = useState(
    r.market_cents != null ? String(r.market_cents / 100) : ""
  );
  const [note, setNote] = useState(r.market_note ?? "");
  const [decision, setDecision] = useState("");
  const refresh = () =>
    void qc.invalidateQueries({ queryKey: ["related-party"] });
  const save = useMutation({
    mutationFn: () =>
      family.note(r.id, {
        market_cents: market ? dollarsToCents(market) : null,
        market_note: note || null,
      }),
    onSuccess: () => {
      toast.success("Saved");
      refresh();
    },
    onError: (e) => toast.error(why(e)),
  });
  const decide = useMutation({
    mutationFn: (approve: boolean) =>
      family.decide(r.id, approve, decision || undefined),
    onSuccess: (d) => {
      toast.success(d.status === "approved" ? "Approved" : "Rejected");
      refresh();
    },
    onError: (e) => toast.error(why(e)),
  });
  const open = r.status === "open";
  const diff =
    r.amount_cents != null && r.market_cents != null
      ? r.amount_cents - r.market_cents
      : null;
  return (
    <Panel className="space-y-4 p-5">
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <Badge>{SUBJECT[r.subject_type]}</Badge>
            <span className="text-[15px] font-semibold text-fg">
              {r.summary}
            </span>
            {r.status === "approved" && <Badge tone="good">Approved</Badge>}
            {r.status === "rejected" && <Badge tone="bad">Rejected</Badge>}
          </div>
          <p className="text-[13px] text-fg-3">
            {r.reason}
            {r.entity_name && ` Paid by ${r.entity_name}.`}
          </p>
          {r.parties.length > 0 && (
            <p className="text-[13px] text-fg-3">
              Parties: {r.parties.map((p) => p.name).join(", ")}
            </p>
          )}
        </div>
        <div className="text-right">
          {r.amount_cents != null && (
            <div className="font-mono text-[18px] font-semibold text-fg">
              {usd(r.amount_cents)}
            </div>
          )}
          {diff != null && (
            <div
              className={
                diff > 0 ? "text-[12px] text-bad" : "text-[12px] text-good"
              }
            >
              {diff > 0
                ? `${usd(diff)} over market`
                : diff < 0
                  ? `${usd(-diff)} under market`
                  : "At market"}
            </div>
          )}
          {r.subject_type === "vendor_bill" && (
            <Link
              href="/console/payables"
              className="text-[12px] text-accent hover:underline"
            >
              Open bills
            </Link>
          )}
        </div>
      </div>

      {open ? (
        <div className="grid gap-3 md:grid-cols-[160px_1fr]">
          <F label="Market price ($)">
            <input
              className={input}
              inputMode="decimal"
              value={market}
              disabled={!can("payable:manage")}
              onChange={(e) => setMarket(e.target.value)}
            />
          </F>
          <F label="Market-rate note">
            <textarea
              rows={2}
              className={input}
              placeholder="Outside quotes, rent comps or an appraisal"
              value={note}
              disabled={!can("payable:manage")}
              onChange={(e) => setNote(e.target.value)}
            />
          </F>
          {can("payable:manage") && (
            <div className="md:col-span-2">
              <Button
                size="sm"
                variant="secondary"
                loading={save.isPending}
                onClick={() => save.mutate()}
              >
                Save note
              </Button>
            </div>
          )}
        </div>
      ) : (
        <dl className="space-y-1 text-[13px]">
          {r.market_note && (
            <p className="text-fg-2">
              <span className="text-fg-3">Market: </span>
              {r.market_cents != null && `${usd(r.market_cents)}. `}
              {r.market_note}
            </p>
          )}
          {r.decision_note && (
            <p className="text-fg-2">
              <span className="text-fg-3">Decision: </span>
              {r.decision_note}
            </p>
          )}
          {r.decided_at && (
            <p className="text-fg-3">
              Decided {new Date(r.decided_at).toLocaleDateString()}
            </p>
          )}
        </dl>
      )}

      {open && can("payable:approve") && (
        <div className="space-y-2 border-t border-line pt-4">
          {r.cannot_decide ? (
            <p className="text-[13px] text-fg-3">{r.cannot_decide}</p>
          ) : (
            <>
              <input
                className={input}
                placeholder="Why (optional)"
                value={decision}
                onChange={(e) => setDecision(e.target.value)}
              />
              <div className="flex gap-2">
                <Button
                  size="sm"
                  disabled={!r.market_note}
                  loading={decide.isPending && decide.variables}
                  onClick={() => decide.mutate(true)}
                >
                  Approve
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  loading={decide.isPending && !decide.variables}
                  onClick={() => decide.mutate(false)}
                >
                  Reject
                </Button>
                {!r.market_note && (
                  <span className="self-center text-[12px] text-fg-3">
                    Save a market-rate note first.
                  </span>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </Panel>
  );
}
