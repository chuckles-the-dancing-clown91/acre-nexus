"use client";

// One-press updates on a work order. Each button posts its note on the
// ticket (the resident sees the public ones) and moves the status when the
// step implies it: "Waiting on parts" puts it on hold, "Work complete"
// resolves it.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  CalendarClock,
  CheckCircle2,
  DoorClosed,
  Lock,
  MapPin,
  Navigation,
  Package,
  PackageCheck,
  Stethoscope,
  Truck,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import { desk, type TicketAction } from "@/lib/servicedesk";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { Panel } from "@/components/ui/panel";

const ICONS: Record<string, typeof Zap> = {
  on_my_way: Navigation,
  arrived: MapPin,
  diagnosed: Stethoscope,
  need_access: DoorClosed,
  waiting_parts: Package,
  parts_in: PackageCheck,
  waiting_vendor: Truck,
  follow_up: CalendarClock,
  work_done: CheckCircle2,
};

const CLOSED = ["resolved", "closed", "cancelled"];

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

export function TicketActions({
  ticketId,
  status,
  onChange,
}: {
  ticketId: string;
  status: string;
  onChange: () => void;
}) {
  const actions = useQuery({
    queryKey: ["ticket-actions"],
    queryFn: desk.actions,
    staleTime: Infinity,
  });
  const [open, setOpen] = useState<TicketAction | null>(null);
  const [note, setNote] = useState("");
  const [date, setDate] = useState("");
  const [busy, setBusy] = useState(false);
  const closed = CLOSED.includes(status);

  async function press(a: TicketAction, extra?: { note?: string }) {
    setBusy(true);
    try {
      await desk.press(ticketId, {
        action: a.key,
        note: extra?.note?.trim() || undefined,
        follow_up_date: a.key === "follow_up" && date ? date : undefined,
      });
      toast.success(a.label);
      setOpen(null);
      setNote("");
      setDate("");
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't post it");
    } finally {
      setBusy(false);
    }
  }

  // A press with nothing to add goes straight through; ones that need a
  // note or a date open a short form first.
  function start(a: TicketAction) {
    if (a.needs_note || a.key === "follow_up") setOpen(a);
    else void press(a);
  }

  const list = (actions.data ?? []).filter(
    (a) => !closed || a.key === "follow_up"
  );
  if (!list.length) return null;

  return (
    <Panel className="p-3">
      <div className="flex flex-wrap gap-2">
        {list.map((a) => {
          const Icon = ICONS[a.key] ?? Zap;
          return (
            <Button
              key={a.key}
              size="sm"
              variant={a.key === "work_done" ? "primary" : "secondary"}
              disabled={busy}
              onClick={() => start(a)}
              title={
                a.visibility === "internal"
                  ? `${a.says} Staff only.`
                  : `${a.says} The resident sees this.`
              }
            >
              <Icon />
              {a.label}
              {a.visibility === "internal" && (
                <Lock className="size-3 opacity-60" />
              )}
            </Button>
          );
        })}
      </div>

      <Dialog open={!!open} onOpenChange={(o) => !o && setOpen(null)}>
        <DialogContent>
          {open && (
            <>
              <DialogTitle className="text-[17px] font-semibold">
                {open.label}
              </DialogTitle>
              <DialogDescription className="mt-1 text-[13px] text-fg-3">
                {open.visibility === "public"
                  ? "Posted on the work order. The resident sees it."
                  : "Posted on the work order for staff."}
              </DialogDescription>
              <div className="mt-4 space-y-3">
                {open.key === "follow_up" && (
                  <label className="block text-xs text-fg-3">
                    Come back on
                    <input
                      type="date"
                      className={`${field} mt-1`}
                      value={date}
                      onChange={(e) => setDate(e.target.value)}
                    />
                  </label>
                )}
                <textarea
                  className={`${field} min-h-[88px]`}
                  autoFocus
                  placeholder={
                    open.key === "diagnosed"
                      ? "What you found, e.g. Door gasket torn; replacing it."
                      : "Add a line (optional)"
                  }
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
              </div>
              <div className="mt-5 flex justify-end gap-2">
                <Button variant="ghost" onClick={() => setOpen(null)}>
                  Cancel
                </Button>
                <Button
                  disabled={busy || (open.needs_note && !note.trim())}
                  onClick={() => press(open, { note })}
                >
                  Post
                </Button>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>
    </Panel>
  );
}
