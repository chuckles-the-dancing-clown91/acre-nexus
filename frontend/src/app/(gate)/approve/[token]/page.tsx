"use client";

// Where an owner's approval link lands: the work, the cost, the photos, and
// approve or decline (sign off or dispute) in one tap. No sign-in.

import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { Check, CheckCircle2, MapPin, ShieldCheck, X } from "lucide-react";
import { toast } from "sonner";
import { owner, type PublicApproval } from "@/lib/owner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function ApprovePage() {
  const { token } = useParams<{ token: string }>();
  const q = useQuery({
    queryKey: ["approve", token],
    queryFn: () => owner.publicView(token),
    retry: false,
  });
  const [view, setView] = useState<PublicApproval | null>(null);
  const a = view ?? q.data;
  const [note, setNote] = useState("");
  const [declining, setDeclining] = useState(false);
  const [busy, setBusy] = useState(false);

  async function answer(approve: boolean) {
    setBusy(true);
    try {
      setView(
        await owner.publicDecide(token, approve, note.trim() || undefined)
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send that");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="mx-auto min-h-dvh max-w-lg px-4 py-8">
      {q.isLoading && <Skeleton className="h-64" />}
      {q.error && (
        <Panel className="p-6 text-center">
          <div className="text-[17px] font-semibold text-fg">
            This link isn&apos;t valid any more
          </div>
          <p className="mt-2 text-[13px] text-fg-3">
            Check your latest message from your property manager.
          </p>
        </Panel>
      )}
      {a && (
        <Panel className="p-6">
          <div className="eyebrow">{a.company}</div>
          <h1 className="mt-1 text-[22px] leading-tight font-semibold text-fg">
            {a.kind === "approval"
              ? `Approve ${a.amount_label} of work?`
              : `${a.title}: done`}
          </h1>
          <p className="mt-1 text-[14px] text-fg-2">{a.title}</p>
          <p className="mt-1 flex items-center gap-1.5 text-[13px] text-fg-3">
            <MapPin className="size-4" />
            {a.property}
          </p>
          {a.kind === "approval" && a.limit_label && (
            <p className="mt-2 text-[12px] text-fg-3">
              Estimated at {a.amount_label}, over your {a.limit_label} limit.
              Nothing goes to a vendor until you say so.
            </p>
          )}
          {a.kind === "signoff" && (
            <p className="mt-2 text-[12px] text-fg-3">
              The work came to {a.amount_label}.
            </p>
          )}
          {a.description && (
            <p className="mt-3 text-[13px] text-fg-2">{a.description}</p>
          )}
          {a.note && (
            <p className="mt-3 rounded-xl bg-fill/60 px-3 py-2 text-[13px] text-fg-2">
              From the office: {a.note}
            </p>
          )}
          {a.tasks.length > 0 && (
            <>
              <div className="mt-4 text-[13px] font-medium text-fg-2">
                {a.kind === "approval" ? "What's planned" : "What was done"}
              </div>
              <ul className="mt-1 list-disc space-y-0.5 pl-5 text-[13px] text-fg">
                {a.tasks.map((t) => (
                  <li key={t}>{t}</li>
                ))}
              </ul>
            </>
          )}
          {a.updates.length > 0 && (
            <>
              <div className="mt-4 text-[13px] font-medium text-fg-2">
                Updates along the way
              </div>
              <ul className="mt-1 space-y-0.5 text-[12px] text-fg-2">
                {a.updates.map((u, i) => (
                  <li key={i}>{u}</li>
                ))}
              </ul>
            </>
          )}
          {a.photos.length > 0 && (
            <ul className="mt-4 grid grid-cols-3 gap-2">
              {a.photos.map((p) =>
                p.url && p.kind === "photo" ? (
                  <li
                    key={p.id}
                    className="aspect-square overflow-hidden rounded-lg bg-fill"
                  >
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                      src={p.url}
                      alt={p.filename}
                      className="size-full object-cover"
                    />
                  </li>
                ) : null
              )}
            </ul>
          )}

          {a.status !== "pending" ? (
            <div
              className={cn(
                "mt-6 flex items-start gap-3 rounded-xl border p-4",
                a.status === "approved"
                  ? "border-good/30 bg-good/10"
                  : "border-warn/30 bg-warn/10"
              )}
            >
              <CheckCircle2
                className={cn(
                  "mt-0.5 size-5",
                  a.status === "approved" ? "text-good" : "text-warn"
                )}
              />
              <p className="text-[14px] text-fg">
                {a.status === "approved"
                  ? a.kind === "approval"
                    ? "Approved. The office is on it."
                    : "Signed off. Thank you."
                  : a.status === "declined"
                    ? "Declined. The office will be in touch."
                    : a.status === "disputed"
                      ? "Noted. The office will look into it and get back to you."
                      : `This was settled already (${a.status}).`}
                {a.decision_note && <> &ldquo;{a.decision_note}&rdquo;</>}
              </p>
            </div>
          ) : declining ? (
            <div className="mt-6 space-y-2">
              <textarea
                className={cn(field, "min-h-[64px] w-full")}
                placeholder={
                  a.kind === "approval"
                    ? "Why not, or what you'd rather do (optional)"
                    : "What's not right?"
                }
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
              <div className="flex flex-col gap-2">
                <Button
                  variant="danger"
                  disabled={busy}
                  onClick={() => answer(false)}
                >
                  {a.kind === "approval" ? "Decline this work" : "Dispute it"}
                </Button>
                <Button variant="ghost" onClick={() => setDeclining(false)}>
                  Back
                </Button>
              </div>
            </div>
          ) : (
            <div className="mt-6 flex flex-col gap-2">
              <Button disabled={busy} onClick={() => answer(true)}>
                <Check />
                {a.kind === "approval"
                  ? `Approve ${a.amount_label}`
                  : "Sign off"}
              </Button>
              <Button
                variant="secondary"
                disabled={busy}
                onClick={() => setDeclining(true)}
              >
                <X />
                {a.kind === "approval" ? "Decline" : "Something's not right"}
              </Button>
              <p className="flex items-center justify-center gap-1 text-[11px] text-fg-4">
                <ShieldCheck className="size-3.5" />
                Answering as {a.owner_name}
              </p>
              <Badge className="mx-auto">
                {a.kind === "approval" ? "Approval" : "Sign-off"}
              </Badge>
            </div>
          )}
        </Panel>
      )}
    </main>
  );
}
