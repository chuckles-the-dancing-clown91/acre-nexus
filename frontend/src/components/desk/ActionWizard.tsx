"use client";

// The work order's "add something" wizard. Pick what you're adding (a note,
// a photo or video, an expense, time, a status move, a task), fill in the
// few things it needs, and it lands on the feed.

import { useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowRightLeft,
  Camera,
  ClipboardPlus,
  Clock,
  MessageSquare,
  Receipt,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import {
  desk,
  parseCents,
  tradeLabel,
  TRADES,
  type Task,
  type TicketFile,
} from "@/lib/servicedesk";
import { ask, needsReason, nextStatuses, STATUS_WORDS } from "@/lib/ticketFlow";
import type { MaintenanceTicket } from "@/lib/types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { cn } from "@/lib/utils";

export type WizardAction =
  "note" | "media" | "expense" | "time" | "status" | "task";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

const CARDS: {
  key: WizardAction;
  label: string;
  hint: string;
  icon: typeof Camera;
}[] = [
  {
    key: "note",
    label: "Note",
    hint: "What you found, did or need",
    icon: MessageSquare,
  },
  {
    key: "media",
    label: "Photo or video",
    hint: "Show the work",
    icon: Camera,
  },
  {
    key: "expense",
    label: "Expense",
    hint: "A purchase with its receipt",
    icon: Receipt,
  },
  { key: "time", label: "Time", hint: "Hours worked on this job", icon: Clock },
  {
    key: "status",
    label: "Change status",
    hint: "Move it along, with a note",
    icon: ArrowRightLeft,
  },
  {
    key: "task",
    label: "Task",
    hint: "Another piece of the work",
    icon: ClipboardPlus,
  },
];

export function ActionWizard({
  ticket,
  tasks,
  initial,
  taskId,
  trackTime,
  onClose,
  onDone,
}: {
  ticket: MaintenanceTicket;
  tasks: Task[];
  /** Skip the choosing and open this one. */
  initial?: WizardAction;
  /** Make a note about this task. */
  taskId?: string;
  trackTime: boolean;
  onClose: () => void;
  onDone: () => void;
}) {
  const [action, setAction] = useState<WizardAction | null>(initial ?? null);
  const [busy, setBusy] = useState(false);
  const files = useRef<HTMLInputElement>(null);

  // note / media
  const [body, setBody] = useState("");
  const [internal, setInternal] = useState(true);
  const [about, setAbout] = useState(taskId ?? "");
  const [attached, setAttached] = useState<TicketFile[]>([]);
  // expense
  const [what, setWhat] = useState("");
  const [vendor, setVendor] = useState("");
  const [amount, setAmount] = useState("");
  const [billable, setBillable] = useState(false);
  const [reimburse, setReimburse] = useState(false);
  const [receipt, setReceipt] = useState<TicketFile | null>(null);
  // time
  const [hours, setHours] = useState("");
  const [mins, setMins] = useState("");
  const [timeNote, setTimeNote] = useState("");
  // status
  const [to, setTo] = useState("");
  const [statusNote, setStatusNote] = useState("");
  const [day, setDay] = useState("");
  const [waiting, setWaiting] = useState("parts");
  const [chase, setChase] = useState("");
  const [leaveOpen, setLeaveOpen] = useState("");
  // task
  const [title, setTitle] = useState("");
  const [trade, setTrade] = useState("general");

  const openTasks = tasks.filter(
    (t) => t.status !== "done" && t.status !== "skipped"
  );

  async function upload(list: FileList, kind: "photo" | "receipt") {
    setBusy(true);
    try {
      const added: TicketFile[] = [];
      for (const f of Array.from(list)) {
        const video = f.type.startsWith("video/");
        if (kind === "photo" && !video && !f.type.startsWith("image/")) {
          toast.error(`${f.name} isn't a photo or video`);
          continue;
        }
        if (f.size > (video ? 100 : 25) * 1024 * 1024) {
          toast.error(`${f.name} is over ${video ? 100 : 25} MB`);
          continue;
        }
        added.push(
          await desk.upload(
            ticket.id,
            f,
            kind === "receipt" ? "receipt" : video ? "video" : "photo"
          )
        );
      }
      if (kind === "receipt") setReceipt(added[0] ?? null);
      else setAttached((a) => [...a, ...added]);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
      if (files.current) files.current.value = "";
    }
  }

  async function submit() {
    if (!action) return;
    setBusy(true);
    try {
      if (action === "note" || action === "media") {
        if (action === "note" && !body.trim() && attached.length === 0)
          throw new Error("Write something first");
        if (action === "media" && attached.length === 0)
          throw new Error("Add a photo or video first");
        await desk.note(ticket.id, {
          body: body.trim() || (attached.length ? "Photos" : ""),
          visibility: internal ? "internal" : "public",
          document_ids: attached.map((a) => a.id),
          ...(about ? { task_id: about } : {}),
        });
        toast.success("Added to the feed");
      } else if (action === "expense") {
        const cents = parseCents(amount);
        if (!what.trim() || !cents)
          throw new Error("Say what was bought and how much");
        await desk.addExpense(ticket.id, {
          description: what.trim(),
          amount_cents: cents,
          vendor: vendor.trim() || undefined,
          receipt_document_ids: receipt ? [receipt.id] : [],
          billable_to_owner: billable,
          reimbursable: reimburse,
        });
        toast.success("Expense logged");
      } else if (action === "time") {
        const total = (Number(hours) || 0) * 60 + (Number(mins) || 0);
        if (total < 1) throw new Error("How long did it take?");
        await desk.logTime(ticket.id, {
          minutes: Math.round(total),
          notes: timeNote.trim() || undefined,
        });
        toast.success("Time logged");
      } else if (action === "status") {
        if (!to) throw new Error("Pick where it's going");
        if (needsReason(ticket.status, to) && !statusNote.trim())
          throw new Error(ask(ticket.status, to));
        if (to === "scheduled" && !day && !ticket.due_date)
          throw new Error("Pick the day");
        if (to === "on_hold" && (!chase || !statusNote.trim()))
          throw new Error("Say what you're chasing and when");
        await api.updateTicket(ticket.id, {
          status: to,
          status_note:
            to === "on_hold" ? undefined : statusNote.trim() || undefined,
          ...(to === "scheduled"
            ? { scheduled_for: day || ticket.due_date || undefined }
            : {}),
          ...(to === "on_hold"
            ? {
                waiting_on: waiting,
                follow_up_date: chase,
                follow_up_note: statusNote.trim(),
              }
            : {}),
          ...(to === "resolved" && openTasks.length
            ? { open_tasks_reason: leaveOpen.trim() || undefined }
            : {}),
        });
        toast.success(`Marked ${STATUS_WORDS[to].toLowerCase()}`);
      } else if (action === "task") {
        if (!title.trim()) throw new Error("Name the task");
        await desk.addTask(ticket.id, { title: title.trim(), trade });
        toast.success("Task added");
      }
      onDone();
      onClose();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
      setBusy(false);
    }
  }

  const card = CARDS.find((c) => c.key === action);
  const moves = nextStatuses(ticket.status);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90dvh] max-w-lg overflow-y-auto">
        <DialogTitle className="flex items-center gap-2 text-[17px] font-semibold">
          {action && !initial && (
            <button
              type="button"
              aria-label="Back"
              onClick={() => setAction(null)}
              className="rounded-lg p-1 text-fg-3 hover:bg-fill-2 hover:text-fg"
            >
              <ArrowLeft className="size-4" />
            </button>
          )}
          {card ? card.label : "Add to this work order"}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {card ? card.hint : "What do you want to add?"}
        </DialogDescription>

        {!action && (
          <div className="mt-4 grid grid-cols-2 gap-2">
            {CARDS.map((c) => (
              <button
                key={c.key}
                type="button"
                onClick={() => setAction(c.key)}
                disabled={c.key === "time" && !trackTime}
                className={cn(
                  "flex items-start gap-3 rounded-xl border border-line p-3 text-left transition hover:border-accent hover:bg-accent/5",
                  "disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:border-line disabled:hover:bg-transparent"
                )}
              >
                <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-fill text-fg-2">
                  <c.icon className="size-4" />
                </span>
                <span>
                  <span className="block text-[13px] font-semibold text-fg">
                    {c.label}
                  </span>
                  <span className="block text-xs text-fg-3">
                    {c.key === "time" && !trackTime
                      ? "Time tracking is off"
                      : c.hint}
                  </span>
                </span>
              </button>
            ))}
          </div>
        )}

        {(action === "note" || action === "media") && (
          <div className="mt-4 space-y-3">
            <textarea
              className={cn(field, "min-h-[96px]")}
              autoFocus
              placeholder={
                action === "media"
                  ? "Caption (optional)"
                  : "What you found, what you did, what's next"
              }
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
            {tasks.length > 0 && (
              <label className="block text-xs text-fg-3">
                About a task (optional)
                <select
                  className={cn(field, "mt-1")}
                  value={about}
                  onChange={(e) => setAbout(e.target.value)}
                >
                  <option value="">The whole work order</option>
                  {tasks.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.title}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <input
              ref={files}
              type="file"
              accept="image/*,video/*"
              multiple
              capture={action === "media" ? "environment" : undefined}
              className="hidden"
              onChange={(e) =>
                e.target.files && void upload(e.target.files, "photo")
              }
            />
            {attached.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {attached.map((a) => (
                  <span
                    key={a.id}
                    className="flex items-center gap-1 rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2"
                  >
                    <Camera className="size-3.5" />
                    {a.filename}
                    <button
                      type="button"
                      aria-label={`Remove ${a.filename}`}
                      onClick={() =>
                        setAttached((x) => x.filter((y) => y.id !== a.id))
                      }
                    >
                      <X className="size-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <div className="flex flex-wrap items-center gap-3">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => files.current?.click()}
              >
                <Camera />
                {attached.length ? "Add more" : "Photo or video"}
              </Button>
              <label className="flex items-center gap-1.5 text-xs text-fg-2">
                <input
                  type="checkbox"
                  checked={internal}
                  onChange={(e) => setInternal(e.target.checked)}
                />
                Staff only
              </label>
            </div>
          </div>
        )}

        {action === "expense" && (
          <div className="mt-4 space-y-3">
            <input
              className={field}
              autoFocus
              placeholder="What was bought"
              value={what}
              onChange={(e) => setWhat(e.target.value)}
            />
            <div className="flex gap-2">
              <input
                className={cn(field, "flex-1")}
                placeholder="Store or vendor"
                value={vendor}
                onChange={(e) => setVendor(e.target.value)}
              />
              <input
                className={cn(field, "w-28")}
                placeholder="$0.00"
                inputMode="decimal"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                aria-label="Amount"
              />
            </div>
            <input
              ref={files}
              type="file"
              accept="image/*,application/pdf"
              capture="environment"
              className="hidden"
              onChange={(e) =>
                e.target.files && void upload(e.target.files, "receipt")
              }
            />
            <div className="flex flex-wrap items-center gap-3 text-xs text-fg-2">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => files.current?.click()}
              >
                <Receipt />
                {receipt ? receipt.filename : "Receipt"}
              </Button>
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={billable}
                  onChange={(e) => setBillable(e.target.checked)}
                />
                Bill the owner
              </label>
              <label className="flex items-center gap-1.5">
                <input
                  type="checkbox"
                  checked={reimburse}
                  onChange={(e) => setReimburse(e.target.checked)}
                />
                Paid out of pocket
              </label>
            </div>
          </div>
        )}

        {action === "time" && (
          <div className="mt-4 space-y-3">
            <div className="flex items-end gap-2">
              <label className="block text-xs text-fg-3">
                Hours
                <input
                  className={cn(field, "mt-1 w-24")}
                  inputMode="numeric"
                  autoFocus
                  value={hours}
                  onChange={(e) => setHours(e.target.value.replace(/\D/g, ""))}
                />
              </label>
              <label className="block text-xs text-fg-3">
                Minutes
                <input
                  className={cn(field, "mt-1 w-24")}
                  inputMode="numeric"
                  value={mins}
                  onChange={(e) => setMins(e.target.value.replace(/\D/g, ""))}
                />
              </label>
            </div>
            <input
              className={field}
              placeholder="What you did (optional)"
              value={timeNote}
              onChange={(e) => setTimeNote(e.target.value)}
            />
            <p className="text-xs text-fg-3">
              Counted as time that just ended, at your pay and bill rates.
            </p>
          </div>
        )}

        {action === "status" && (
          <div className="mt-4 space-y-3">
            <div className="text-xs text-fg-3">
              Now{" "}
              <span className="font-medium text-fg-2">
                {STATUS_WORDS[ticket.status]}
              </span>
              . Move it to:
            </div>
            <div className="flex flex-wrap gap-2">
              {moves.map((s) => (
                <button
                  key={s}
                  type="button"
                  onClick={() => setTo(s)}
                  aria-pressed={to === s}
                  className={cn(
                    "rounded-lg border px-3 py-1.5 text-[13px] font-medium transition",
                    to === s
                      ? "border-accent bg-accent/10 text-fg"
                      : "border-line text-fg-2 hover:border-fg-4"
                  )}
                >
                  {STATUS_WORDS[s]}
                </button>
              ))}
            </div>
            {to && (
              <p className="text-xs text-fg-3">{ask(ticket.status, to)}</p>
            )}
            {to === "scheduled" && (
              <input
                type="date"
                className={field}
                value={day || ticket.due_date || ""}
                onChange={(e) => setDay(e.target.value)}
                aria-label="Scheduled for"
              />
            )}
            {to === "on_hold" && (
              <div className="grid grid-cols-2 gap-2">
                <label className="block text-xs text-fg-3">
                  Waiting on
                  <select
                    className={cn(field, "mt-1")}
                    value={waiting}
                    onChange={(e) => setWaiting(e.target.value)}
                  >
                    {["parts", "vendor", "resident", "owner", "other"].map(
                      (w) => (
                        <option key={w} value={w}>
                          {w[0].toUpperCase() + w.slice(1)}
                        </option>
                      )
                    )}
                  </select>
                </label>
                <label className="block text-xs text-fg-3">
                  Chase on
                  <input
                    type="date"
                    className={cn(field, "mt-1")}
                    value={chase}
                    onChange={(e) => setChase(e.target.value)}
                  />
                </label>
              </div>
            )}
            {to === "resolved" && openTasks.length > 0 && (
              <label className="block text-xs text-fg-3">
                {openTasks.length}{" "}
                {openTasks.length === 1 ? "task is" : "tasks are"} still open.
                Finish them, or say why they can stay open:
                <input
                  className={cn(field, "mt-1")}
                  value={leaveOpen}
                  onChange={(e) => setLeaveOpen(e.target.value)}
                />
              </label>
            )}
            {to && (
              <textarea
                className={cn(field, "min-h-[72px]")}
                placeholder={
                  to === "on_hold"
                    ? "What are you chasing?"
                    : needsReason(ticket.status, to)
                      ? "Why"
                      : "Note for the feed (optional)"
                }
                value={statusNote}
                onChange={(e) => setStatusNote(e.target.value)}
              />
            )}
          </div>
        )}

        {action === "task" && (
          <div className="mt-4 space-y-3">
            <input
              className={field}
              autoFocus
              placeholder="e.g. Replace bathroom fan"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
            <select
              className={field}
              value={trade}
              onChange={(e) => setTrade(e.target.value)}
              aria-label="Trade"
            >
              {TRADES.map((t) => (
                <option key={t} value={t}>
                  {tradeLabel(t)}
                </option>
              ))}
            </select>
          </div>
        )}

        {action && (
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button loading={busy} onClick={() => void submit()}>
              {action === "status" ? "Move it" : "Add"}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
