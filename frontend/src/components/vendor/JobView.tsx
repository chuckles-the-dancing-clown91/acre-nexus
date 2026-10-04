"use client";

// One job sent to a vendor: the work order, their tasks, and one place to
// accept (with a time), decline, send photos and the invoice, and say it's
// done. Used by the no-account link and the signed-in vendor portal.

import { useRef, useState } from "react";
import {
  AlertTriangle,
  Camera,
  CalendarCheck,
  Check,
  CheckCircle2,
  FileText,
  MapPin,
  Receipt,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { instantFrom } from "@/lib/appointments";
import { minutesLabel, tradeLabel } from "@/lib/servicedesk";
import { centsFrom, type VendorClient, type VendorJob } from "@/lib/vendorLink";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export function Job({
  client,
  job,
  onChange,
}: {
  client: VendorClient;
  job: VendorJob;
  onChange: (j: VendorJob) => void;
}) {
  const [mode, setMode] = useState<"idle" | "accept" | "decline" | "done">(
    "idle"
  );
  const [note, setNote] = useState("");
  const [date, setDate] = useState("");
  const [time, setTime] = useState("09:00");
  const [busy, setBusy] = useState(false);
  const open = job.response !== "declined" && job.response !== "done";

  async function run(fn: () => Promise<VendorJob>, said: string) {
    setBusy(true);
    try {
      onChange(await fn());
      toast.success(said);
      setMode("idle");
      setNote("");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send that");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      <Panel className="p-6">
        <div className="eyebrow">{job.company}</div>
        <h1 className="mt-1 text-[22px] leading-tight font-semibold text-fg">
          {job.title}
        </h1>
        <p className="mt-1 flex items-center gap-1.5 text-[13px] text-fg-3">
          <MapPin className="size-4 shrink-0" />
          {job.property}
        </p>
        <div className="mt-2 flex flex-wrap items-center gap-1.5 text-[12px]">
          <Badge
            tone={
              job.priority === "urgent" || job.priority === "high"
                ? "warn"
                : "neutral"
            }
          >
            {job.priority} priority
          </Badge>
          {job.due_date && <Badge>Wanted by {job.due_date}</Badge>}
        </div>
        {job.description && (
          <p className="mt-3 text-[14px] whitespace-pre-line text-fg-2">
            {job.description}
          </p>
        )}
        {job.note && (
          <p className="mt-3 rounded-xl bg-fill/60 px-3 py-2 text-[13px] text-fg-2">
            From the office: {job.note}
          </p>
        )}
        {job.access_notes && (
          <p className="mt-2 rounded-xl border border-warn/30 bg-warn/[0.06] px-3 py-2 text-[13px] text-fg-2">
            Getting in: {job.access_notes}
          </p>
        )}

        <div className="mt-5 text-[13px] font-medium text-fg-2">Your tasks</div>
        <ul className="mt-2 divide-y divide-line rounded-xl border border-line">
          {job.tasks.map((t) => (
            <li key={t.id} className="flex items-center gap-3 px-3 py-2.5">
              <span
                className={cn(
                  "flex size-5 shrink-0 items-center justify-center rounded-md border",
                  t.status === "done"
                    ? "border-good bg-good text-white"
                    : "border-line-strong"
                )}
              >
                {t.status === "done" && <Check className="size-3.5" />}
              </span>
              <div className="min-w-0 flex-1">
                <div
                  className={cn(
                    "text-[14px]",
                    t.status === "done" ? "text-fg-3 line-through" : "text-fg"
                  )}
                >
                  {t.title}
                </div>
                <div className="text-[12px] text-fg-3">
                  {tradeLabel(t.trade)}
                  {t.est_minutes
                    ? ` · about ${minutesLabel(t.est_minutes)}`
                    : ""}
                </div>
              </div>
            </li>
          ))}
        </ul>

        {job.response === "declined" && (
          <Status
            icon={<XCircle className="mt-0.5 size-5 text-warn" />}
            tone="warn"
            text="You declined this work. The office will send it to someone else."
          />
        )}
        {job.response === "done" && (
          <Status
            icon={<CheckCircle2 className="mt-0.5 size-5 text-good" />}
            tone="good"
            text="Marked done. Thanks. The office will review it and pay your invoice."
          />
        )}
        {job.response === "accepted" && (
          <Status
            icon={<CalendarCheck className="mt-0.5 size-5 text-good" />}
            tone="good"
            text={
              job.when_words
                ? `You're on for ${job.when_words}. The resident has been told.`
                : "You accepted this work. Let the office know when you're coming."
            }
          />
        )}

        {open && mode === "idle" && (
          <div className="mt-5 grid gap-2">
            {job.response !== "accepted" && (
              <Button onClick={() => setMode("accept")}>
                <Check />
                Accept this work
              </Button>
            )}
            {job.response === "accepted" && (
              <Button onClick={() => setMode("done")}>
                <CheckCircle2 />
                Mark it done
              </Button>
            )}
            <div className="flex gap-2">
              {job.response === "accepted" && (
                <Button
                  variant="secondary"
                  className="flex-1"
                  onClick={() => setMode("accept")}
                >
                  <CalendarCheck />
                  {job.when_words ? "Change the time" : "Say when"}
                </Button>
              )}
              <Button
                variant="ghost"
                className="flex-1"
                onClick={() => setMode("decline")}
              >
                Can&apos;t take it
              </Button>
            </div>
          </div>
        )}

        {mode === "accept" && (
          <div className="mt-5 space-y-2">
            <div className="text-[13px] font-medium text-fg-2">
              When can you come? (optional, but the resident gets told)
            </div>
            <div className="flex flex-wrap gap-2">
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
            <p className="text-[11px] text-fg-4">
              Times are {job.timezone.replace(/_/g, " ")}.
            </p>
            <textarea
              className={cn(field, "min-h-[64px] w-full")}
              placeholder="Anything the office should know (optional)"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
            <div className="flex flex-col gap-2">
              <Button
                disabled={busy}
                onClick={() =>
                  run(
                    () =>
                      client.accept({
                        note: note.trim() || undefined,
                        start: instantFrom(date, time) ?? undefined,
                      }),
                    "Accepted"
                  )
                }
              >
                {busy
                  ? "Sending…"
                  : date
                    ? "Accept and book that time"
                    : "Accept"}
              </Button>
              <Button variant="ghost" onClick={() => setMode("idle")}>
                Back
              </Button>
            </div>
          </div>
        )}

        {mode === "decline" && (
          <div className="mt-5 space-y-2">
            <textarea
              className={cn(field, "min-h-[64px] w-full")}
              placeholder="Why not? (optional, helps the office plan)"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
            <div className="flex flex-col gap-2">
              <Button
                variant="danger"
                disabled={busy}
                onClick={() =>
                  run(
                    () => client.decline(note.trim() || undefined),
                    "Declined"
                  )
                }
              >
                {busy ? "Sending…" : "Decline this work"}
              </Button>
              <Button variant="ghost" onClick={() => setMode("idle")}>
                Back
              </Button>
            </div>
          </div>
        )}

        {mode === "done" && (
          <div className="mt-5 space-y-2">
            <textarea
              className={cn(field, "min-h-[64px] w-full")}
              placeholder="What you did, anything to watch (optional)"
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
            {job.invoices.length === 0 && (
              <p className="flex items-start gap-1.5 text-[12px] text-fg-3">
                <AlertTriangle className="mt-0.5 size-3.5 shrink-0 text-warn" />
                No invoice yet. You can still send one after.
              </p>
            )}
            <div className="flex flex-col gap-2">
              <Button
                disabled={busy}
                onClick={() =>
                  run(
                    () => client.done(note.trim() || undefined),
                    "Marked done"
                  )
                }
              >
                {busy ? "Sending…" : "It's done"}
              </Button>
              <Button variant="ghost" onClick={() => setMode("idle")}>
                Back
              </Button>
            </div>
          </div>
        )}
      </Panel>

      {job.response !== "declined" && (
        <Files client={client} job={job} onChange={onChange} />
      )}
      {job.response !== "declined" && (
        <Invoice client={client} job={job} onChange={onChange} />
      )}
    </div>
  );
}

function Status({
  icon,
  tone,
  text,
}: {
  icon: React.ReactNode;
  tone: "good" | "warn";
  text: string;
}) {
  return (
    <div
      className={cn(
        "mt-5 flex items-start gap-3 rounded-xl border p-4",
        tone === "good"
          ? "border-good/30 bg-good/10"
          : "border-warn/30 bg-warn/10"
      )}
    >
      {icon}
      <p className="text-[14px] text-fg">{text}</p>
    </div>
  );
}

function Files({
  client,
  job,
  onChange,
}: {
  client: VendorClient;
  job: VendorJob;
  onChange: (j: VendorJob) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const photos = job.files.filter((f) => f.kind !== "receipt");

  async function attach(list: FileList) {
    setBusy(true);
    try {
      for (const f of Array.from(list)) {
        if (!f.type.startsWith("image/") && !f.type.startsWith("video/")) {
          toast.error(`${f.name} isn't a photo or video`);
          continue;
        }
        await client.upload(f, "photo");
      }
      onChange(await client.view());
      toast.success("Uploaded");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Upload failed");
    } finally {
      setBusy(false);
      if (input.current) input.current.value = "";
    }
  }

  return (
    <Panel className="p-5">
      <div className="flex items-center justify-between">
        <div className="text-[14px] font-semibold text-fg">Photos</div>
        <Button
          size="sm"
          variant="secondary"
          disabled={busy}
          onClick={() => input.current?.click()}
        >
          <Camera />
          {busy ? "Uploading…" : "Add photos"}
        </Button>
        <input
          ref={input}
          type="file"
          accept="image/*,video/*"
          multiple
          className="hidden"
          aria-label="Photos"
          onChange={(e) => e.target.files && attach(e.target.files)}
        />
      </div>
      <p className="mt-1 text-[12px] text-fg-3">
        Before and after shots go straight on the work order.
      </p>
      {photos.length > 0 && (
        <ul className="mt-3 grid grid-cols-3 gap-2">
          {photos.map((f) => (
            <li
              key={f.id}
              className="aspect-square overflow-hidden rounded-lg bg-fill"
            >
              {f.url && f.kind === "photo" ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img
                  src={f.url}
                  alt={f.filename}
                  className="size-full object-cover"
                />
              ) : (
                <div className="flex size-full items-center justify-center text-[11px] text-fg-3">
                  {f.filename}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

function Invoice({
  client,
  job,
  onChange,
}: {
  client: VendorClient;
  job: VendorJob;
  onChange: (j: VendorJob) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [amount, setAmount] = useState("");
  const [description, setDescription] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const cents = centsFrom(amount);

  async function send() {
    if (!cents) return;
    setBusy(true);
    try {
      let document_id: string | undefined;
      if (file) document_id = (await client.upload(file, "invoice")).id;
      onChange(
        await client.invoice({
          amount_cents: cents,
          description: description.trim() || undefined,
          document_id,
        })
      );
      setAmount("");
      setDescription("");
      setFile(null);
      toast.success("Invoice sent");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className="p-5">
      <div className="text-[14px] font-semibold text-fg">Your invoice</div>
      <p className="mt-1 text-[12px] text-fg-3">
        It goes to the office as a bill on this work order.
      </p>
      {job.invoices.length > 0 && (
        <ul className="mt-3 space-y-1.5">
          {job.invoices.map((i) => (
            <li
              key={i.id}
              className="flex items-center justify-between rounded-lg bg-fill/60 px-3 py-2 text-[13px]"
            >
              <span className="flex items-center gap-1.5 text-fg-2">
                <Receipt className="size-3.5 text-fg-3" />
                {i.description}
              </span>
              <span className="figure font-medium text-fg">
                {i.amount_label}
              </span>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        <input
          className={cn(field, "w-32")}
          inputMode="decimal"
          placeholder="$0.00"
          aria-label="Amount"
          value={amount}
          onChange={(e) => setAmount(e.target.value)}
        />
        <input
          className={cn(field, "min-w-0 flex-1")}
          placeholder="What it covers"
          aria-label="Description"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </div>
      <div className="mt-2 flex items-center gap-2">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => input.current?.click()}
        >
          <FileText />
          {file ? file.name : "Attach the PDF or photo"}
        </Button>
        <input
          ref={input}
          type="file"
          accept="application/pdf,image/*"
          className="hidden"
          aria-label="Invoice file"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
        />
      </div>
      <Button className="mt-3 w-full" disabled={!cents || busy} onClick={send}>
        {busy ? "Sending…" : "Send invoice"}
      </Button>
    </Panel>
  );
}
