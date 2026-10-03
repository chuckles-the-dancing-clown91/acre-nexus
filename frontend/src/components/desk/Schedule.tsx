"use client";

// Scheduling the visit for a work order: offer up to four windows, see
// what the resident picked or asked for, confirm a time agreed by phone,
// and mark how the visit went.

import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CalendarCheck,
  CalendarClock,
  CalendarX,
  Copy,
  Phone,
  Plus,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  appointments,
  instantFrom,
  statusWords,
  type Appointment,
  type WindowInput,
} from "@/lib/appointments";
import type { MaintenanceTicket } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-lg border border-line bg-surface px-2.5 py-1.5 text-[13px] text-fg outline-none focus:border-accent";

type Row = { date: string; start: string; end: string };

const blank = (): Row => ({ date: "", start: "09:00", end: "11:00" });

function tone(s: string): "good" | "warn" | "bad" | "neutral" | "info" {
  switch (s) {
    case "confirmed":
      return "good";
    case "proposed":
      return "info";
    case "declined":
      return "warn";
    case "no_show":
      return "bad";
    default:
      return "neutral";
  }
}

export function Schedule({
  ticket,
  manage,
  onChange,
}: {
  ticket: MaintenanceTicket;
  manage: boolean;
  onChange: () => void;
}) {
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["appointments", "ticket", ticket.id],
    queryFn: () =>
      appointments.list({ subject_type: "ticket", subject_id: ticket.id }),
  });
  const [offering, setOffering] = useState(false);
  const [rows, setRows] = useState<Row[]>([blank()]);
  const [note, setNote] = useState("");
  const [phoneAt, setPhoneAt] = useState<Row | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["appointments"] });
    onChange();
  };
  const all = list.data ?? [];
  const current =
    all.find((a) => a.status === "confirmed" || a.status === "proposed") ??
    all.find((a) => a.status === "declined") ??
    null;
  const past = all.filter((a) => a.id !== current?.id);
  const closed = ["resolved", "closed", "cancelled"].includes(ticket.status);

  function toWindows(rs: Row[]): WindowInput[] | null {
    const out: WindowInput[] = [];
    for (const r of rs) {
      if (!r.date) continue;
      const start = instantFrom(r.date, r.start);
      const end = instantFrom(r.date, r.end);
      if (!start || !end) return null;
      out.push({ start, end });
    }
    return out;
  }

  async function offer() {
    const windows = toWindows(rows);
    if (!windows || windows.length === 0) {
      toast.error("Give at least one date and time.");
      return;
    }
    setBusy(true);
    try {
      const a = await appointments.offer({
        ticket_id: ticket.id,
        windows,
        note: note.trim() || undefined,
      });
      await navigator.clipboard?.writeText(a.link).catch(() => undefined);
      toast.success(
        a.with_email || a.with_phone
          ? "Times sent. The link is on your clipboard too."
          : "Times offered."
      );
      setOffering(false);
      setRows([blank()]);
      setNote("");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  async function confirmByPhone() {
    if (!current || !phoneAt) return;
    const w = toWindows([phoneAt]);
    if (!w?.length) {
      toast.error("Give the date and time you agreed on.");
      return;
    }
    setBusy(true);
    try {
      await appointments.update(current.id, { confirm: w[0] });
      toast.success("Confirmed");
      setPhoneAt(null);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't confirm it");
    } finally {
      setBusy(false);
    }
  }

  async function mark(
    a: Appointment,
    status: "cancelled" | "done" | "no_show"
  ) {
    try {
      await appointments.update(a.id, { status });
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save it");
    }
  }

  async function copyLink(a: Appointment) {
    // The link is only handed out on creation; a fresh offer is the way to
    // send it again.
    toast.message(
      a.with_email || a.with_phone
        ? `Sent to ${a.with_name ?? "the resident"}${a.with_email ? ` at ${a.with_email}` : ""}.`
        : "Nobody to send it to; add an email or phone on the lease."
    );
  }

  return (
    <Panel>
      <PanelHeader
        title="Visit"
        description={
          current
            ? current.status === "confirmed"
              ? `Confirmed for ${current.when_words}`
              : current.status === "proposed"
                ? "Waiting on a pick."
                : "They need a different time."
            : "Nothing scheduled yet."
        }
        action={
          manage &&
          !closed && (
            <Button
              size="sm"
              variant={current?.status === "confirmed" ? "ghost" : "secondary"}
              onClick={() => setOffering(true)}
            >
              <CalendarClock />
              {current ? "Offer new times" : "Offer times"}
            </Button>
          )
        }
      />
      <div className="space-y-3 p-5 pt-4">
        {current ? (
          <Visit
            a={current}
            manage={manage}
            onMark={(s) => mark(current, s)}
            onPhone={() => setPhoneAt(blank())}
            onCopy={() => copyLink(current)}
          />
        ) : (
          <p className="text-[13px] text-fg-3">
            {manage
              ? "Offer a few windows; the resident picks one from a text or the portal, and reminders go out before."
              : "No visit has been offered yet."}
          </p>
        )}
        {past.length > 0 && (
          <details className="text-xs text-fg-3">
            <summary className="cursor-pointer">
              {past.length} earlier {past.length === 1 ? "visit" : "visits"}
            </summary>
            <ul className="mt-2 space-y-1">
              {past.map((a) => (
                <li key={a.id} className="flex items-center gap-2">
                  <Badge tone={tone(a.status)}>
                    {statusWords(a.status, a.with_role)}
                  </Badge>
                  <span>{a.when_words ?? a.windows_words.join("; ")}</span>
                  {a.outcome_note && <span>· {a.outcome_note}</span>}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>

      <Dialog open={offering} onOpenChange={setOffering}>
        <DialogContent className="max-w-lg">
          <DialogTitle className="text-[17px] font-semibold">
            Offer times for the visit
          </DialogTitle>
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            Up to four. They pick one from a text or email, or in the portal.
            {current?.status === "proposed" &&
              " This replaces the times offered before."}
          </DialogDescription>
          <div className="mt-4 space-y-2">
            {rows.map((r, i) => (
              <div key={i} className="flex flex-wrap items-center gap-2">
                <input
                  type="date"
                  className={field}
                  aria-label={`Window ${i + 1} date`}
                  value={r.date}
                  onChange={(e) =>
                    setRows(
                      rows.map((x, j) =>
                        j === i ? { ...x, date: e.target.value } : x
                      )
                    )
                  }
                />
                <input
                  type="time"
                  className={field}
                  aria-label={`Window ${i + 1} start`}
                  value={r.start}
                  onChange={(e) =>
                    setRows(
                      rows.map((x, j) =>
                        j === i ? { ...x, start: e.target.value } : x
                      )
                    )
                  }
                />
                <span className="text-xs text-fg-3">to</span>
                <input
                  type="time"
                  className={field}
                  aria-label={`Window ${i + 1} end`}
                  value={r.end}
                  onChange={(e) =>
                    setRows(
                      rows.map((x, j) =>
                        j === i ? { ...x, end: e.target.value } : x
                      )
                    )
                  }
                />
                {rows.length > 1 && (
                  <button
                    type="button"
                    aria-label={`Remove window ${i + 1}`}
                    onClick={() => setRows(rows.filter((_, j) => j !== i))}
                    className="rounded-lg p-1.5 text-fg-3 hover:text-bad"
                  >
                    <Trash2 className="size-4" />
                  </button>
                )}
              </div>
            ))}
            {rows.length < 4 && (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() =>
                  setRows([
                    ...rows,
                    { ...blank(), date: rows[rows.length - 1].date },
                  ])
                }
              >
                <Plus />
                Another window
              </Button>
            )}
            <textarea
              className={cn(field, "min-h-[56px] w-full")}
              placeholder="A note for them (optional): what we'll do, how long it takes"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setOffering(false)}>
              Cancel
            </Button>
            <Button onClick={offer} disabled={busy}>
              {busy ? "Sending…" : "Send times"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={!!phoneAt} onOpenChange={(o) => !o && setPhoneAt(null)}>
        <DialogContent className="max-w-md">
          <DialogTitle className="text-[17px] font-semibold">
            Confirm a time you agreed on
          </DialogTitle>
          <DialogDescription className="mt-1 text-[13px] text-fg-3">
            For when it was settled on the phone. They get a confirmation.
          </DialogDescription>
          {phoneAt && (
            <div className="mt-4 flex flex-wrap items-center gap-2">
              <input
                type="date"
                className={field}
                aria-label="Date"
                value={phoneAt.date}
                onChange={(e) =>
                  setPhoneAt({ ...phoneAt, date: e.target.value })
                }
              />
              <input
                type="time"
                className={field}
                aria-label="Start"
                value={phoneAt.start}
                onChange={(e) =>
                  setPhoneAt({ ...phoneAt, start: e.target.value })
                }
              />
              <span className="text-xs text-fg-3">to</span>
              <input
                type="time"
                className={field}
                aria-label="End"
                value={phoneAt.end}
                onChange={(e) =>
                  setPhoneAt({ ...phoneAt, end: e.target.value })
                }
              />
            </div>
          )}
          <div className="mt-5 flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setPhoneAt(null)}>
              Cancel
            </Button>
            <Button onClick={confirmByPhone} disabled={busy}>
              <Phone />
              Confirm
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </Panel>
  );
}

function Visit({
  a,
  manage,
  onMark,
  onPhone,
  onCopy,
}: {
  a: Appointment;
  manage: boolean;
  onMark: (s: "cancelled" | "done" | "no_show") => void;
  onPhone: () => void;
  onCopy: () => void;
}) {
  const open = a.status === "proposed" || a.status === "confirmed";
  return (
    <div className="rounded-xl border border-line p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone={tone(a.status)}>
          {statusWords(a.status, a.with_role)}
        </Badge>
        <span className="text-[13px] text-fg">
          {a.with_name ?? "The resident"}
          {a.with_phone && <span className="text-fg-3"> · {a.with_phone}</span>}
        </span>
        {a.assignee_name && (
          <span className="text-xs text-fg-3">· {a.assignee_name} going</span>
        )}
      </div>
      {a.status === "confirmed" && a.when_words && (
        <div className="mt-2 flex items-center gap-2 text-[14px] font-medium text-fg">
          <CalendarCheck className="size-4 text-good" />
          {a.when_words}
          {a.confirmed_by && (
            <span className="text-xs font-normal text-fg-3">
              picked by{" "}
              {a.confirmed_by === "staff" ? "us" : `the ${a.confirmed_by}`}
            </span>
          )}
        </div>
      )}
      {a.status === "proposed" && (
        <ul className="mt-2 space-y-1 text-[13px] text-fg-2">
          {a.windows_words.map((w) => (
            <li key={w} className="flex items-center gap-2">
              <CalendarClock className="size-3.5 text-fg-3" />
              {w}
            </li>
          ))}
        </ul>
      )}
      {a.status === "declined" && (
        <div className="mt-2 flex items-start gap-2 text-[13px] text-fg">
          <CalendarX className="mt-0.5 size-4 text-warn" />
          <div>
            None of the offered times work.
            {a.proposed_words && (
              <div className="font-medium">
                They asked for {a.proposed_words}.
              </div>
            )}
            {a.outcome_note && (
              <div className="text-fg-3">“{a.outcome_note}”</div>
            )}
          </div>
        </div>
      )}
      {a.note && (
        <div className="mt-2 text-xs text-fg-3">Note sent: {a.note}</div>
      )}
      {manage && (
        <div className="mt-3 flex flex-wrap gap-1.5">
          {open && (
            <Button size="sm" variant="ghost" onClick={onPhone}>
              <Phone />
              {a.status === "confirmed" ? "Change time" : "Confirm by phone"}
            </Button>
          )}
          {a.status === "confirmed" && (
            <>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => onMark("done")}
              >
                Visit done
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => onMark("no_show")}
              >
                Nobody home
              </Button>
            </>
          )}
          {open && (
            <Button
              size="sm"
              variant="ghost"
              onClick={() => onMark("cancelled")}
            >
              Cancel visit
            </Button>
          )}
          {a.status === "proposed" && (
            <Button size="sm" variant="ghost" onClick={onCopy}>
              <Copy />
              Where it went
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
