"use client";

// The owner's home: what's waiting on them first, then their properties,
// the month so far, and the open work. Built for a phone.

import { useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Building2,
  Check,
  ChevronRight,
  FileText,
  ShieldCheck,
  Wrench,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { ApiError } from "@/lib/api";
import { approvalWords, owner, type OwnerApproval } from "@/lib/owner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function OwnerHomePage() {
  const home = useQuery({
    queryKey: ["owner", "home"],
    queryFn: owner.home,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  if (home.isLoading) return <Skeleton className="h-64" />;
  if (home.error) {
    const none = home.error instanceof ApiError && home.error.status === 404;
    return (
      <Panel>
        <EmptyState
          icon={<Building2 />}
          title={
            none ? "No properties on file yet" : "Couldn't load your portal"
          }
          description={
            none
              ? "Once your manager links your properties to this email, they show here."
              : home.error.message
          }
        />
      </Panel>
    );
  }
  const h = home.data!;
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">
          Hi, {h.name.split(" ")[0]}
        </h1>
        <p className="text-[13px] text-fg-3">
          {h.properties.length} propert{h.properties.length === 1 ? "y" : "ies"}
          {h.entities.length ? ` across ${h.entities.join(", ")}` : ""}.
        </p>
      </div>

      {h.pending.length > 0 && (
        <section className="space-y-2">
          <h2 className="text-[13px] font-medium text-fg-2">Waiting on you</h2>
          {h.pending.map((a) => (
            <ApprovalCard key={a.id} a={a} />
          ))}
        </section>
      )}

      <Panel className="p-4">
        <div className="flex items-baseline justify-between">
          <div className="text-[13px] font-medium text-fg-2">
            So far this month
          </div>
          <Link
            href={`/account/owner/statement?month=${h.last_month}`}
            className="flex items-center gap-1 text-[12px] text-accent"
          >
            <FileText className="size-3.5" />
            Last month&apos;s statement
          </Link>
        </div>
        <div className="mt-3 grid grid-cols-3 gap-3">
          <Figure
            label="Rent collected"
            value={h.month_to_date.rent_collected_label}
          />
          <Figure label="Expenses" value={h.month_to_date.expenses_label} />
          <Figure label="Net" value={h.month_to_date.net_label} strong />
        </div>
        <p className="mt-3 text-[11px] text-fg-4">
          Work over {h.approval_limit_label} waits for your approval before it
          starts.
        </p>
      </Panel>

      <section className="space-y-2">
        <h2 className="text-[13px] font-medium text-fg-2">Your properties</h2>
        {h.properties.map((p) => (
          <Panel key={p.id} className="flex items-center gap-3 p-3">
            {p.image_url ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={p.image_url}
                alt=""
                className="size-14 rounded-lg object-cover"
              />
            ) : (
              <div className="flex size-14 items-center justify-center rounded-lg bg-fill">
                <Building2 className="size-5 text-fg-3" />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px] font-semibold text-fg">
                {p.name}
              </div>
              <div className="truncate text-[12px] text-fg-3">{p.address}</div>
              <div className="mt-0.5 text-[12px] text-fg-2">
                {p.units > 1
                  ? `${p.occupied_units} of ${p.units} units rented`
                  : p.occupied_units
                    ? "Rented"
                    : "Vacant"}
                {" · "}
                {p.monthly_rent_label}/mo
                {p.open_work > 0 &&
                  ` · ${p.open_work} open job${p.open_work === 1 ? "" : "s"}`}
              </div>
            </div>
          </Panel>
        ))}
      </section>

      <section className="space-y-2">
        <h2 className="text-[13px] font-medium text-fg-2">Open work</h2>
        {h.open_work.length === 0 && (
          <p className="text-[13px] text-fg-3">Nothing open right now.</p>
        )}
        {h.open_work.map((w) => (
          <Panel key={w.id} className="flex items-center gap-3 p-3">
            <Wrench className="size-4 shrink-0 text-fg-3" />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px] text-fg">{w.title}</div>
              <div className="truncate text-[12px] text-fg-3">
                {w.property} · {w.status.replace("_", " ")}
                {w.waiting_on ? `, waiting on ${w.waiting_on}` : ""} · est.{" "}
                {w.est_label}
              </div>
            </div>
            {w.approval && (
              <Badge
                tone={
                  w.approval.status === "pending"
                    ? "warn"
                    : w.approval.status === "approved"
                      ? "good"
                      : "neutral"
                }
              >
                {approvalWords(w.approval)}
              </Badge>
            )}
          </Panel>
        ))}
      </section>
    </div>
  );
}

function Figure({
  label,
  value,
  strong,
}: {
  label: string;
  value: string;
  strong?: boolean;
}) {
  return (
    <div>
      <div className="text-[11px] text-fg-3">{label}</div>
      <div
        className={cn(
          "figure text-[18px] font-semibold",
          strong ? "text-good" : "text-fg"
        )}
      >
        {value}
      </div>
    </div>
  );
}

/** One ask with approve and decline right on it. */
export function ApprovalCard({ a }: { a: OwnerApproval }) {
  const qc = useQueryClient();
  const [note, setNote] = useState("");
  const [declining, setDeclining] = useState(false);
  const [busy, setBusy] = useState(false);
  async function answer(approve: boolean) {
    setBusy(true);
    try {
      await owner.decide(a.id, approve, note.trim() || undefined);
      toast.success(
        approve ? (a.kind === "approval" ? "Approved" : "Signed off") : "Sent"
      );
      qc.invalidateQueries({ queryKey: ["owner"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send that");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel className="border-warn/40 p-4">
      <div className="flex items-start gap-3">
        <ShieldCheck className="mt-0.5 size-5 shrink-0 text-warn" />
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-semibold text-fg">
            {a.kind === "approval"
              ? `Approve ${a.amount_label} of work?`
              : `Sign off on ${a.amount_label} of finished work?`}
          </div>
          <div className="text-[13px] text-fg-2">{a.ticket_title}</div>
          <div className="flex items-center gap-1 text-[12px] text-fg-3">
            {a.property}
            <ChevronRight className="size-3" />
            asked {new Date(a.requested_at).toLocaleDateString()}
          </div>
          {a.note && (
            <p className="mt-1 text-[12px] text-fg-2">&ldquo;{a.note}&rdquo;</p>
          )}
        </div>
      </div>
      {declining ? (
        <div className="mt-3 space-y-2">
          <textarea
            className={cn(field, "min-h-[56px] w-full")}
            placeholder={
              a.kind === "approval"
                ? "Why not, or what you'd rather do (optional)"
                : "What's not right?"
            }
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <div className="flex gap-2">
            <Button
              variant="danger"
              className="flex-1"
              disabled={busy}
              onClick={() => answer(false)}
            >
              {a.kind === "approval" ? "Decline" : "Dispute it"}
            </Button>
            <Button variant="ghost" onClick={() => setDeclining(false)}>
              Back
            </Button>
          </div>
        </div>
      ) : (
        <div className="mt-3 flex gap-2">
          <Button
            className="flex-1"
            disabled={busy}
            onClick={() => answer(true)}
          >
            <Check />
            {a.kind === "approval" ? "Approve" : "Sign off"}
          </Button>
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() => setDeclining(true)}
          >
            <X />
            {a.kind === "approval" ? "Decline" : "Not right"}
          </Button>
        </div>
      )}
    </Panel>
  );
}
