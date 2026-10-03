"use client";

// The page a scheduling link opens: the visit, the times offered, one tap
// to pick, or a way to ask for another time. No sign-in.

import { useState } from "react";
import { useParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { CalendarCheck, CalendarClock, Check, MapPin } from "lucide-react";
import { toast } from "sonner";
import { appointments, instantFrom } from "@/lib/appointments";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function BookPage() {
  const { token } = useParams<{ token: string }>();
  const q = useQuery({
    queryKey: ["book", token],
    queryFn: () => appointments.publicView(token),
    retry: false,
  });
  const [picked, setPicked] = useState<number | null>(null);
  const [other, setOther] = useState(false);
  const [date, setDate] = useState("");
  const [time, setTime] = useState("09:00");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<string | null>(null);

  async function pick() {
    if (picked == null) return;
    setBusy(true);
    try {
      const a = await appointments.publicPick(token, picked);
      setDone(`You're set for ${a.when_words}. We'll remind you before.`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    } finally {
      setBusy(false);
    }
  }

  async function decline() {
    setBusy(true);
    try {
      const start = instantFrom(date, time);
      await appointments.publicDecline(token, {
        propose: start ? { start } : undefined,
        reason: reason.trim() || undefined,
      });
      setDone(
        start
          ? "Thanks. We'll check that time and confirm with you."
          : "Thanks for letting us know. We'll be in touch with other times."
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
            It may have been replaced by newer times. Check your latest message
            from us.
          </p>
        </Panel>
      )}
      {q.data && (
        <Panel className="p-6">
          <div className="eyebrow">{q.data.company}</div>
          <h1 className="mt-1 text-[22px] leading-tight font-semibold text-fg">
            {q.data.title}
          </h1>
          <p className="mt-1 flex items-center gap-1.5 text-[13px] text-fg-3">
            <MapPin className="size-4" />
            {q.data.property}
          </p>
          {q.data.note && (
            <p className="mt-3 rounded-xl bg-fill/60 px-3 py-2 text-[13px] text-fg-2">
              {q.data.note}
            </p>
          )}

          {done ? (
            <div className="mt-6 flex items-start gap-3 rounded-xl border border-good/30 bg-good/10 p-4">
              <CalendarCheck className="mt-0.5 size-5 text-good" />
              <p className="text-[14px] text-fg">{done}</p>
            </div>
          ) : q.data.status === "confirmed" ? (
            <div className="mt-6 flex items-start gap-3 rounded-xl border border-good/30 bg-good/10 p-4">
              <CalendarCheck className="mt-0.5 size-5 text-good" />
              <p className="text-[14px] text-fg">
                You&apos;re set for {q.data.when_words}.
              </p>
            </div>
          ) : q.data.status !== "proposed" ? (
            <p className="mt-6 text-[14px] text-fg-2">
              This visit is no longer open for scheduling.
            </p>
          ) : (
            <>
              <div className="mt-6 text-[13px] font-medium text-fg-2">
                {other ? "Suggest a time that works" : "Pick a time that works"}
              </div>
              {!other ? (
                <>
                  <ul className="mt-2 space-y-2">
                    {q.data.windows_words.map((w, i) => (
                      <li key={w}>
                        <button
                          type="button"
                          onClick={() => setPicked(i)}
                          className={cn(
                            "flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left text-[14px] transition",
                            picked === i
                              ? "border-accent bg-accent/10 text-fg"
                              : "border-line text-fg-2 hover:bg-fill"
                          )}
                        >
                          <span
                            className={cn(
                              "flex size-5 shrink-0 items-center justify-center rounded-full border",
                              picked === i
                                ? "border-accent bg-accent text-accent-fg"
                                : "border-line-strong"
                            )}
                          >
                            {picked === i && <Check className="size-3" />}
                          </span>
                          <CalendarClock className="size-4 shrink-0 text-fg-3" />
                          {w}
                        </button>
                      </li>
                    ))}
                  </ul>
                  <p className="mt-2 text-[11px] text-fg-4">
                    Times are {q.data.timezone.replace(/_/g, " ")}.
                  </p>
                  <div className="mt-5 flex flex-col gap-2">
                    <Button onClick={pick} disabled={picked == null || busy}>
                      {busy ? "Saving…" : "Confirm this time"}
                    </Button>
                    <Button variant="ghost" onClick={() => setOther(true)}>
                      None of these work
                    </Button>
                  </div>
                </>
              ) : (
                <>
                  <div className="mt-2 flex flex-wrap gap-2">
                    <input
                      type="date"
                      className={field}
                      aria-label="Date"
                      value={date}
                      onChange={(e) => setDate(e.target.value)}
                    />
                    <input
                      type="time"
                      className={field}
                      aria-label="Time"
                      value={time}
                      onChange={(e) => setTime(e.target.value)}
                    />
                  </div>
                  <textarea
                    className={cn(field, "mt-2 min-h-[64px] w-full")}
                    placeholder="Anything we should know (optional)"
                    value={reason}
                    onChange={(e) => setReason(e.target.value)}
                  />
                  <div className="mt-4 flex flex-col gap-2">
                    <Button onClick={decline} disabled={busy}>
                      {busy ? "Sending…" : date ? "Suggest this time" : "Send"}
                    </Button>
                    <Button variant="ghost" onClick={() => setOther(false)}>
                      Back to the offered times
                    </Button>
                  </div>
                </>
              )}
            </>
          )}
        </Panel>
      )}
    </main>
  );
}
