"use client";

// One task, opened: what it is, who has it, what the vendor said, and its own
// feed of notes and updates. Mark it done, send it to a vendor, or add a note
// about it.

import { useState } from "react";
import {
  CheckCircle2,
  HardHat,
  Mail,
  MessageSquarePlus,
  Send,
  User,
} from "lucide-react";
import { toast } from "sonner";
import { vendorResponseWords } from "@/lib/vendorLink";
import { desk, minutesLabel, tradeLabel, type Task } from "@/lib/servicedesk";
import { Feed } from "@/components/desk/Feed";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";

const STATUS_TONE = {
  todo: "neutral",
  doing: "info",
  done: "good",
  skipped: "neutral",
} as const;

const when = (iso: string | null) =>
  iso
    ? new Date(iso).toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "numeric",
        minute: "2-digit",
      })
    : null;

export function TaskDetail({
  ticketId,
  task: t,
  manage,
  onClose,
  onChange,
  onSend,
  onNote,
}: {
  ticketId: string;
  task: Task;
  manage: boolean;
  onClose: () => void;
  onChange: () => void;
  onSend: (t: Task) => void;
  onNote?: (taskId: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [bump, setBump] = useState(0);

  async function set(status: Task["status"]) {
    setBusy(true);
    try {
      await desk.updateTask(ticketId, t.id, { status });
      toast.success(
        status === "done"
          ? "Marked done"
          : status === "skipped"
            ? "Skipped"
            : "Reopened"
      );
      setBump((n) => n + 1);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  const facts: [string, React.ReactNode][] = [
    ["Time estimate", t.est_minutes ? minutesLabel(t.est_minutes) : null],
    ["Cost estimate", t.est_cost_label],
    [
      "Team member",
      t.assignee_user_name ? (
        <span className="inline-flex items-center gap-1">
          <User className="size-3.5" />
          {t.assignee_user_name}
        </span>
      ) : null,
    ],
    [
      "Vendor",
      t.assignee_name ? (
        <span className="inline-flex items-center gap-1">
          {t.dispatch_via === "email" ? (
            <Mail className="size-3.5" />
          ) : (
            <HardHat className="size-3.5" />
          )}
          {t.assignee_name}
          {t.dispatch_via === "partner" && " (their board)"}
          {t.dispatch_via === "email" && " (email)"}
        </span>
      ) : null,
    ],
    ["Sent", when(t.dispatched_at)],
    ["Done", when(t.done_at)],
  ];

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-xl overflow-y-auto">
        <DialogTitle className="flex flex-wrap items-center gap-2 text-[17px] font-semibold">
          {t.title}
          <Badge tone={STATUS_TONE[t.status]}>
            {t.status === "todo" ? "To do" : t.status}
          </Badge>
          <Badge>{tradeLabel(t.trade)}</Badge>
        </DialogTitle>
        <DialogDescription className="sr-only">
          Details, notes and vendor updates for this task.
        </DialogDescription>

        <dl className="mt-4 grid grid-cols-2 gap-x-6 gap-y-2 text-[13px]">
          {facts
            .filter(([, v]) => v)
            .map(([k, v]) => (
              <div key={k}>
                <dt className="text-xs text-fg-3">{k}</dt>
                <dd className="text-fg">{v}</dd>
              </div>
            ))}
        </dl>

        {(t.vendor_response || t.dispatch_note) && (
          <div className="mt-4 space-y-2 rounded-xl border border-line bg-fill/40 p-3 text-[13px]">
            {t.dispatch_note && (
              <p className="text-fg-2">
                <span className="text-fg-3">You told them: </span>
                {t.dispatch_note}
              </p>
            )}
            {t.vendor_response && (
              <p className="text-fg-2">
                <span className="text-fg-3">Vendor: </span>
                <Badge
                  tone={
                    t.vendor_response === "declined"
                      ? "warn"
                      : t.vendor_response === "done"
                        ? "info"
                        : "good"
                  }
                >
                  {vendorResponseWords(t.vendor_response)}
                </Badge>
                {t.vendor_responded_at && (
                  <span className="text-fg-3">
                    {" "}
                    · {when(t.vendor_responded_at)}
                  </span>
                )}
                {t.vendor_note && (
                  <span className="block pt-1 text-fg">
                    &ldquo;{t.vendor_note}&rdquo;
                  </span>
                )}
              </p>
            )}
          </div>
        )}

        {manage && (
          <div className="mt-4 flex flex-wrap gap-2">
            {t.status === "done" ? (
              <Button
                size="sm"
                variant="secondary"
                loading={busy}
                onClick={() => void set("todo")}
              >
                Reopen
              </Button>
            ) : (
              <>
                <Button
                  size="sm"
                  loading={busy}
                  onClick={() => void set("done")}
                >
                  <CheckCircle2 />
                  Mark done
                </Button>
                {t.status !== "skipped" && (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={() => void set("skipped")}
                  >
                    Skip
                  </Button>
                )}
              </>
            )}
            <Button size="sm" variant="secondary" onClick={() => onSend(t)}>
              <Send />
              Send to a vendor
            </Button>
            {onNote && (
              <Button
                size="sm"
                variant="secondary"
                onClick={() => onNote(t.id)}
              >
                <MessageSquarePlus />
                Add a note
              </Button>
            )}
          </div>
        )}

        <div className="mt-5 border-t border-line pt-4">
          <div className="eyebrow mb-3">Notes and updates</div>
          <Feed
            ticketId={ticketId}
            taskId={t.id}
            refreshKey={bump}
            empty="No notes on this task yet."
          />
        </div>
      </DialogContent>
    </Dialog>
  );
}
