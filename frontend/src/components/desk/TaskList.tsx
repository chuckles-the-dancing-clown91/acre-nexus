"use client";

// The tasks on a work order: tick them off, add one, send contractor work to a
// vendor, or pull in a whole kit.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Check,
  HardHat,
  Mail,
  PackagePlus,
  Plus,
  Send,
  Trash2,
  User,
} from "lucide-react";
import { toast } from "sonner";
import { ApiError } from "@/lib/api";
import { vendorResponseWords } from "@/lib/vendorLink";
import {
  desk,
  loadLabel,
  minutesLabel,
  tradeLabel,
  TRADES,
  type Task,
  type VendorOption,
} from "@/lib/servicedesk";
import { KitPreview } from "@/components/desk/KitPreview";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-lg border border-line bg-surface px-2.5 py-1.5 text-[13px] text-fg outline-none focus:border-accent";

export function TaskList({
  ticketId,
  propertyId,
  tasks,
  manage,
  onChange,
}: {
  ticketId: string;
  /** For offering the people on this property first. */
  propertyId?: string;
  tasks: Task[];
  manage: boolean;
  onChange: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const [title, setTitle] = useState("");
  const [trade, setTrade] = useState("general");
  const [minutes, setMinutes] = useState("");
  const [contractor, setContractor] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dispatching, setDispatching] = useState<Task[] | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const techs = useQuery({
    queryKey: ["techs", propertyId ?? "all"],
    queryFn: () => desk.techs(propertyId),
    enabled: manage,
  });
  const [kitOpen, setKitOpen] = useState(false);

  async function run(fn: () => Promise<unknown>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    } finally {
      setBusy(false);
    }
  }

  const done = tasks.filter((t) => t.status === "done").length;
  const counted = tasks.filter((t) => t.status !== "skipped").length;

  return (
    <Panel>
      <PanelHeader
        title="Tasks"
        description={
          tasks.length
            ? `${done} of ${counted} done`
            : "The work, line by line."
        }
        action={
          manage && (
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setKitOpen(true)}
              >
                <PackagePlus />
                Add kit
              </Button>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setAdding(true)}
              >
                <Plus />
                Task
              </Button>
            </div>
          )
        }
      />
      {manage && picked.length > 0 && (
        <div className="mx-5 mt-3 flex items-center gap-2 rounded-xl border border-accent/30 bg-accent/10 px-3 py-2 text-[13px]">
          <span className="font-medium text-fg">{picked.length} selected</span>
          <Button
            size="sm"
            className="ml-auto"
            onClick={() =>
              setDispatching(tasks.filter((t) => picked.includes(t.id)))
            }
          >
            <Send />
            Send to a vendor
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setPicked([])}>
            Clear
          </Button>
        </div>
      )}
      {counted > 0 && (
        <div className="mx-5 mt-3 h-1.5 overflow-hidden rounded-full bg-fill">
          <div
            className="h-full rounded-full bg-accent transition-[width] duration-500"
            style={{ width: `${Math.round((done * 100) / counted)}%` }}
          />
        </div>
      )}
      <div className="p-2 pt-3">
        {tasks.length === 0 && !adding && (
          <EmptyState
            icon={<Check />}
            title="No tasks yet"
            description={
              manage ? "Add tasks one by one, or pull in a job kit." : undefined
            }
            className="py-8"
          />
        )}
        <ul className="divide-y divide-line">
          {tasks.map((t) => (
            <li key={t.id} className="flex items-center gap-3 px-3 py-2.5">
              {manage && t.status !== "done" && t.status !== "skipped" && (
                <input
                  type="checkbox"
                  aria-label={`Select ${t.title}`}
                  checked={picked.includes(t.id)}
                  onChange={(e) =>
                    setPicked((p) =>
                      e.target.checked
                        ? [...p, t.id]
                        : p.filter((x) => x !== t.id)
                    )
                  }
                  className="size-4 shrink-0 accent-[var(--accent)]"
                />
              )}
              <button
                type="button"
                disabled={!manage || busy}
                onClick={() =>
                  run(() =>
                    desk.updateTask(ticketId, t.id, {
                      status: t.status === "done" ? "todo" : "done",
                    })
                  )
                }
                aria-label={
                  t.status === "done"
                    ? `Reopen ${t.title}`
                    : `Mark ${t.title} done`
                }
                className={cn(
                  "flex size-5 shrink-0 items-center justify-center rounded-md border transition",
                  t.status === "done"
                    ? "border-accent bg-accent text-accent-fg"
                    : "border-line-strong hover:border-accent"
                )}
              >
                {t.status === "done" && <Check className="size-3.5" />}
              </button>
              <div className="min-w-0 flex-1">
                <div
                  className={cn(
                    "truncate text-[13px]",
                    t.status === "done" ? "text-fg-3 line-through" : "text-fg"
                  )}
                >
                  {t.title}
                </div>
                <div className="flex flex-wrap items-center gap-x-2 text-xs text-fg-3">
                  {t.est_minutes ? (
                    <span>{minutesLabel(t.est_minutes)}</span>
                  ) : null}
                  {t.est_cost_label && <span>{t.est_cost_label}</span>}
                  {t.assignee_name && (
                    <span className="inline-flex items-center gap-1 text-fg-2">
                      {t.dispatch_via === "email" ? (
                        <Mail className="size-3" />
                      ) : (
                        <HardHat className="size-3" />
                      )}
                      {t.dispatched_at ? "Sent to " : "Vendor: "}
                      {t.assignee_name}
                      {t.dispatch_via === "partner" && " (their board)"}
                      {t.dispatch_via === "email" && " (email)"}
                    </span>
                  )}
                  {t.vendor_response && (
                    <Badge
                      tone={
                        t.vendor_response === "declined"
                          ? "warn"
                          : t.vendor_response === "done"
                            ? "info"
                            : "good"
                      }
                      className="h-[18px]"
                    >
                      {vendorResponseWords(t.vendor_response)}
                    </Badge>
                  )}
                  {t.assignee_user_name && (
                    <span className="inline-flex items-center gap-1 text-fg-2">
                      <User className="size-3" />
                      {t.assignee_user_name}
                    </span>
                  )}
                </div>
                {t.dispatch_note && (
                  <div className="truncate text-xs text-fg-4">
                    Note: {t.dispatch_note}
                  </div>
                )}
                {t.vendor_note && (
                  <div className="truncate text-xs text-fg-4">
                    Vendor said: &ldquo;{t.vendor_note}&rdquo;
                  </div>
                )}
              </div>
              {t.needs_contractor && (
                <HardHat
                  className={cn(
                    "size-4 shrink-0",
                    t.assignee_entity_id ? "text-good" : "text-warn"
                  )}
                  aria-label={
                    t.assignee_entity_id
                      ? "Vendor assigned"
                      : "Needs a contractor"
                  }
                />
              )}
              <Badge>{tradeLabel(t.trade)}</Badge>
              {manage && !t.needs_contractor && !t.assignee_entity_id && (
                <select
                  aria-label={`Who does ${t.title}`}
                  value={t.assignee_user_id ?? ""}
                  disabled={busy}
                  onChange={(e) =>
                    run(() =>
                      desk.updateTask(ticketId, t.id, {
                        assignee_user_id: e.target.value,
                      })
                    )
                  }
                  className="hidden max-w-[9rem] rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2 sm:block"
                >
                  <option value="">Unassigned</option>
                  {techs.data?.map((p) => (
                    <option key={p.user_id} value={p.user_id}>
                      {p.name} ({loadLabel(p)})
                    </option>
                  ))}
                </select>
              )}
              {manage && (
                <>
                  <button
                    type="button"
                    onClick={() => setDispatching([t])}
                    disabled={busy}
                    aria-label={`Send ${t.title} to a vendor`}
                    className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-accent"
                  >
                    <Send className="size-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      if (confirm(`Remove "${t.title}"?`))
                        void run(() => desk.removeTask(ticketId, t.id));
                    }}
                    disabled={busy}
                    aria-label={`Remove ${t.title}`}
                    className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-bad"
                  >
                    <Trash2 className="size-4" />
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
        {adding && (
          <form
            className="m-2 flex flex-wrap items-center gap-2 rounded-xl border border-line bg-fill/40 p-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (!title.trim()) return;
              void run(async () => {
                await desk.addTask(ticketId, {
                  title: title.trim(),
                  trade,
                  est_minutes: Number(minutes) || undefined,
                  needs_contractor: contractor,
                });
                setTitle("");
                setMinutes("");
                setContractor(false);
                setAdding(false);
              });
            }}
          >
            <input
              className={cn(field, "min-w-0 flex-1 basis-56")}
              placeholder="e.g. Replace bathroom fan"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
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
            <input
              className={cn(field, "w-24")}
              placeholder="Minutes"
              inputMode="numeric"
              value={minutes}
              onChange={(e) => setMinutes(e.target.value.replace(/\D/g, ""))}
              aria-label="Estimated minutes"
            />
            <label className="flex items-center gap-1.5 text-xs text-fg-2">
              <input
                type="checkbox"
                checked={contractor}
                onChange={(e) => setContractor(e.target.checked)}
              />
              Contractor
            </label>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => setAdding(false)}
            >
              Cancel
            </Button>
            <Button type="submit" size="sm" disabled={busy || !title.trim()}>
              Add
            </Button>
          </form>
        )}
      </div>

      {dispatching && (
        <DispatchDialog
          ticketId={ticketId}
          tasks={dispatching}
          onClose={() => setDispatching(null)}
          onSent={() => {
            setDispatching(null);
            setPicked([]);
            onChange();
          }}
        />
      )}
      {kitOpen && (
        <KitDialog
          ticketId={ticketId}
          onClose={() => setKitOpen(false)}
          onAdded={() => {
            setKitOpen(false);
            onChange();
          }}
        />
      )}
    </Panel>
  );
}

/** Choose a vendor for a task (those covering its trade first) and send it. */
function DispatchDialog({
  ticketId,
  tasks,
  onClose,
  onSent,
}: {
  ticketId: string;
  tasks: Task[];
  onClose: () => void;
  onSent: () => void;
}) {
  const task = tasks[0];
  // Vendors are matched on the trade most of the tasks share.
  const trades = [...new Set(tasks.map((t) => t.trade))];
  const trade = trades.length === 1 ? task.trade : undefined;
  const vendors = useQuery({
    queryKey: ["vendors", ticketId, trade ?? "any"],
    queryFn: () => desk.vendors(ticketId, trade),
  });
  const [vendorId, setVendorId] = useState(
    tasks.length === 1 ? (task.assignee_entity_id ?? "") : ""
  );
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [invited, setInvited] = useState<string[]>([]);

  async function invite(v: VendorOption) {
    try {
      await desk.alphaInvite(v.id);
      setInvited((i) => [...i, v.id]);
      toast.success(`Invited ${v.name} to Alpha`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send the invite");
    }
  }

  async function send() {
    if (!vendorId) return;
    setBusy(true);
    const go = (reason?: string) =>
      desk.dispatchTasks(ticketId, {
        task_ids: tasks.map((t) => t.id),
        entity_id: vendorId,
        note: note.trim() || undefined,
        coi_override_reason: reason,
      });
    try {
      try {
        await go();
      } catch (e) {
        if (!(e instanceof ApiError) || e.status !== 409) throw e;
        const reason = window.prompt(
          `${e.message}\n\nReason to send them anyway:`
        );
        if (!reason?.trim()) {
          setBusy(false);
          return;
        }
        await go(reason.trim());
      }
      toast.success("Sent to the vendor");
      onSent();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Send to a vendor
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {tasks.length === 1
            ? `${task.title} · ${tradeLabel(task.trade)}.`
            : `${tasks.length} tasks, sent as one job.`}{" "}
          Linked vendors get it on their own job board; everyone else gets an
          email with a link to accept, schedule, and send their invoice.
        </DialogDescription>
        {tasks.length > 1 && (
          <ul className="mt-3 space-y-0.5 rounded-xl border border-line bg-fill/40 px-3 py-2 text-xs text-fg-2">
            {tasks.map((t) => (
              <li key={t.id} className="flex justify-between gap-2">
                <span className="truncate">{t.title}</span>
                <span className="shrink-0 text-fg-3">
                  {tradeLabel(t.trade)}
                </span>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-4 max-h-64 space-y-1.5 overflow-y-auto">
          {vendors.data?.length === 0 && (
            <p className="text-[13px] text-fg-3">
              No contractors yet. Add one under Entities, with the trades they
              cover.
            </p>
          )}
          {vendors.data?.map((v) => (
            <label
              key={v.id}
              className={cn(
                "flex cursor-pointer items-center gap-3 rounded-xl border px-3 py-2.5 transition",
                vendorId === v.id
                  ? "border-accent bg-accent/10"
                  : "border-line hover:bg-fill-2"
              )}
            >
              <input
                type="radio"
                name="vendor"
                className="sr-only"
                checked={vendorId === v.id}
                onChange={() => setVendorId(v.id)}
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-fg">
                  {v.name}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {v.trades.map(tradeLabel).join(", ") || "No trades listed"}
                </div>
              </div>
              {v.matches && trade && (
                <Badge tone="good">{tradeLabel(trade)}</Badge>
              )}
              {v.linked && <Badge tone="info">Linked</Badge>}
              {!v.coi_current && <Badge tone="warn">No COI</Badge>}
              {!v.linked &&
                v.email &&
                (v.alpha_invited_at || invited.includes(v.id) ? (
                  <span className="text-[11px] text-fg-4">Invited</span>
                ) : (
                  <button
                    type="button"
                    className="text-[11px] text-accent hover:underline"
                    onClick={(e) => {
                      e.preventDefault();
                      invite(v);
                    }}
                  >
                    Invite to Alpha
                  </button>
                ))}
            </label>
          ))}
        </div>
        <textarea
          className="mt-3 min-h-[64px] w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent"
          placeholder="Note for the vendor (access, timing, preferences)"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={send} disabled={!vendorId || busy}>
            <Send />
            {busy ? "Sending…" : "Send"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** Pull a whole job kit's tasks and parts onto this work order. */
function KitDialog({
  ticketId,
  onClose,
  onAdded,
}: {
  ticketId: string;
  onClose: () => void;
  onAdded: () => void;
}) {
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  const [kitId, setKitId] = useState("");
  const [busy, setBusy] = useState(false);
  const kit = kits.data?.find((k) => k.id === kitId);
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogTitle className="text-[17px] font-semibold">
          Add a job kit
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Its tasks and parts go on after what&apos;s already here.
        </DialogDescription>
        <select
          className="mt-4 w-full rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg"
          value={kitId}
          onChange={(e) => setKitId(e.target.value)}
          aria-label="Kit"
        >
          <option value="">Choose a kit…</option>
          {kits.data?.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
              {k.est_total_cents ? ` · ${k.est_total_label}` : ""}
            </option>
          ))}
        </select>
        {kit && (
          <div className="mt-4 max-h-[50vh] overflow-y-auto pr-1">
            <KitPreview kit={kit} />
          </div>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            disabled={!kitId || busy}
            onClick={async () => {
              setBusy(true);
              try {
                await desk.applyKit(ticketId, kitId);
                toast.success("Kit added");
                onAdded();
              } catch (e) {
                toast.error(e instanceof Error ? e.message : "Couldn't add it");
                setBusy(false);
              }
            }}
          >
            Add kit
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
