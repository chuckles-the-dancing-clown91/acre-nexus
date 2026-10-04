"use client";

// The owner's say on a work order: who they are, their limit against the
// estimate, the asks so far, and the buttons to ask them or record an
// answer given by phone.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Send, ShieldCheck, X } from "lucide-react";
import { toast } from "sonner";
import { approvalWords, owner, type OwnerApproval as Ask } from "@/lib/owner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function tone(a: Ask): "good" | "warn" | "bad" | "neutral" | "info" {
  switch (a.status) {
    case "pending":
      return "warn";
    case "approved":
      return "good";
    case "declined":
    case "disputed":
      return "bad";
    case "overridden":
      return "info";
    default:
      return "neutral";
  }
}

export function OwnerApprovalPanel({
  ticketId,
  manage,
  onChange,
}: {
  ticketId: string;
  manage: boolean;
  onChange: () => void;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["ticket-approvals", ticketId],
    queryFn: () => owner.forTicket(ticketId),
  });
  const [note, setNote] = useState("");
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const d = q.data;
  if (!d || !d.owner_id) return null;
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["ticket-approvals", ticketId] });
    onChange();
  };
  const pending = d.approvals.find((a) => a.status === "pending");

  async function ask() {
    setBusy(true);
    try {
      await owner.request(ticketId, { note: note.trim() || undefined });
      toast.success(`Asked ${d!.owner_name}`);
      setAsking(false);
      setNote("");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send the ask");
    } finally {
      setBusy(false);
    }
  }
  async function record(a: Ask, approve: boolean) {
    const what = approve
      ? "approved"
      : a.kind === "signoff"
        ? "disputed"
        : "declined";
    const n = window.prompt(
      `Record that ${d!.owner_name} ${what} this (what they said, optional):`,
      ""
    );
    if (n === null) return;
    setBusy(true);
    try {
      await owner.staffDecide(a.id, approve, n.trim() || undefined);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't record that");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className={cn(d.needs_approval && "border-warn/40")}>
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            <ShieldCheck
              className={cn(
                "size-4",
                d.needs_approval ? "text-warn" : "text-fg-3"
              )}
            />
            Owner: {d.owner_name}
            {d.needs_approval && <Badge tone="warn">Needs approval</Badge>}
          </span>
        }
        description={
          d.limit_cents > 0
            ? `Estimate ${d.est_total_label} against their ${d.limit_label} limit.`
            : `No spend limit; work goes out without asking.`
        }
        action={
          manage &&
          !pending &&
          !asking && (
            <Button
              size="sm"
              variant={d.needs_approval ? "primary" : "secondary"}
              onClick={() => setAsking(true)}
            >
              <Send />
              Ask the owner
            </Button>
          )
        }
      />
      {asking && (
        <div className="space-y-2 px-5 pb-4">
          <textarea
            className="min-h-[56px] w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent"
            placeholder={`A line for ${d.owner_name} (optional)`}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => setAsking(false)}>
              Cancel
            </Button>
            <Button size="sm" disabled={busy} onClick={ask}>
              <Send />
              {busy ? "Sending…" : `Ask for ${d.est_total_label}`}
            </Button>
          </div>
        </div>
      )}
      {d.approvals.length > 0 && (
        <ul className="divide-y divide-line border-t border-line">
          {d.approvals.map((a) => (
            <li
              key={a.id}
              className="flex items-start gap-3 px-5 py-2.5 text-[13px]"
            >
              <Badge tone={tone(a)} className="mt-0.5 shrink-0">
                {approvalWords(a)}
              </Badge>
              <div className="min-w-0 flex-1">
                <div className="text-fg">
                  {a.kind === "approval" ? "Approval" : "Sign-off"} for{" "}
                  {a.amount_label}
                  <span className="text-fg-3">
                    {" "}
                    · asked {new Date(a.requested_at).toLocaleDateString()}
                    {a.decided_at &&
                      `, answered ${new Date(a.decided_at).toLocaleDateString()} by ${a.decided_by}`}
                    {a.nudges > 0 && `, reminded ${a.nudges}×`}
                  </span>
                </div>
                {a.note && (
                  <div className="text-[12px] text-fg-3">
                    To the owner: {a.note}
                  </div>
                )}
                {a.decision_note && (
                  <div className="text-[12px] text-fg-2">
                    They said: &ldquo;{a.decision_note}&rdquo;
                  </div>
                )}
                {a.override_reason && (
                  <div className="text-[12px] text-fg-2">
                    Went ahead because: &ldquo;{a.override_reason}&rdquo;
                  </div>
                )}
              </div>
              {manage && a.status === "pending" && (
                <span className="flex shrink-0 gap-1">
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => record(a, true)}
                    title="They said yes by phone"
                  >
                    <Check />
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => record(a, false)}
                    title="They said no by phone"
                  >
                    <X />
                  </Button>
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
