"use client";

// Showings on a phone. Today's and upcoming showings with the prospect's
// number one tap away; new leads to book; after the walk-through, send the
// application; once it's approved, write the lease and send it to sign.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CalendarPlus,
  Check,
  Copy,
  DoorOpen,
  FileSignature,
  Mail,
  MapPin,
  MessageSquare,
  Phone,
  Plus,
  Send,
  UserPlus,
} from "lucide-react";
import { toast } from "sonner";
import { api, type EsignSignerLink, type Lead } from "@/lib/api";
import {
  appointments,
  instantFrom,
  ymd,
  type Appointment,
} from "@/lib/appointments";
import { useAuth } from "@/lib/auth";
import {
  dollarsToCents,
  leadStage,
  mapLink,
  phoneLinks,
  showings,
} from "@/lib/showings";
import type { Application, Property } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-[14px] text-fg outline-none focus:border-accent";

export default function ShowingsPage() {
  const { can } = useAuth();
  const qc = useQueryClient();
  const today = ymd(new Date());
  const horizon = useMemo(() => {
    const d = new Date();
    d.setDate(d.getDate() + 30);
    return ymd(d);
  }, []);
  const list = useQuery({
    queryKey: ["appointments", "showings", today],
    queryFn: () => appointments.list({ from: today, to: horizon }),
    enabled: can("maintenance:read"),
  });
  const leads = useQuery({
    queryKey: ["leads"],
    queryFn: () => api.leads(),
    enabled: can("application:read"),
  });
  const apps = useQuery({
    queryKey: ["applications"],
    queryFn: api.applications,
    enabled: can("application:read"),
  });
  const properties = useQuery({
    queryKey: ["properties"],
    queryFn: api.properties,
  });
  const [booking, setBooking] = useState<Lead | null>(null);
  const [adding, setAdding] = useState(false);
  const [leasing, setLeasing] = useState<{
    lead: Lead;
    application: Application;
    property_id?: string;
  } | null>(null);

  const showingsList = (list.data ?? []).filter(
    (a) =>
      a.kind === "showing" && a.status !== "cancelled" && a.status !== "done"
  );
  const todays = showingsList.filter(
    (a) => (a.starts_at ?? a.windows[0]?.start ?? "").slice(0, 10) === today
  );
  const later = showingsList.filter((a) => !todays.includes(a));
  const leadById = new Map((leads.data?.leads ?? []).map((l) => [l.id, l]));
  const appById = new Map((apps.data ?? []).map((a) => [a.id, a]));
  const openLeads = (leads.data?.leads ?? []).filter(
    (l) =>
      l.status !== "closed" && !showingsList.some((a) => a.subject_id === l.id)
  );
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["appointments"] });
    qc.invalidateQueries({ queryKey: ["leads"] });
    qc.invalidateQueries({ queryKey: ["applications"] });
  };

  return (
    <div className="mx-auto max-w-xl space-y-5">
      <PageHeader
        eyebrow="Leasing"
        title="Showings"
        description="Walk them through, send the application, send the lease."
        actions={
          can("application:write") && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setAdding(true)}
            >
              <UserPlus />
              New lead
            </Button>
          )
        }
      />

      {list.isLoading && <Skeleton className="h-40" />}

      <Section title="Today" count={todays.length}>
        {todays.length === 0 && !list.isLoading && (
          <p className="text-[13px] text-fg-3">No showings today.</p>
        )}
        {todays.map((a) => (
          <ShowingCard
            key={a.id}
            a={a}
            lead={a.subject_id ? leadById.get(a.subject_id) : undefined}
            app={
              a.subject_id
                ? appById.get(leadById.get(a.subject_id)?.application_id ?? "")
                : undefined
            }
            onChange={refresh}
            onLease={(lead, application) =>
              setLeasing({ lead, application, property_id: a.property_id })
            }
          />
        ))}
      </Section>

      <Section title="Coming up" count={later.length}>
        {later.length === 0 && !list.isLoading && (
          <p className="text-[13px] text-fg-3">Nothing booked ahead.</p>
        )}
        {later.map((a) => (
          <ShowingCard
            key={a.id}
            a={a}
            lead={a.subject_id ? leadById.get(a.subject_id) : undefined}
            app={
              a.subject_id
                ? appById.get(leadById.get(a.subject_id)?.application_id ?? "")
                : undefined
            }
            onChange={refresh}
            onLease={(lead, application) =>
              setLeasing({ lead, application, property_id: a.property_id })
            }
          />
        ))}
      </Section>

      <Section title="Leads to book" count={openLeads.length}>
        {openLeads.length === 0 && !leads.isLoading && (
          <p className="text-[13px] text-fg-3">Every lead has a showing.</p>
        )}
        {openLeads.map((l) => {
          const app = l.application_id
            ? appById.get(l.application_id)
            : undefined;
          const phone = phoneLinks(l.phone);
          return (
            <Panel key={l.id} className="p-4">
              <div className="flex items-start gap-3">
                <div className="min-w-0 flex-1">
                  <div className="text-[15px] font-semibold text-fg">
                    {l.name}
                  </div>
                  <div className="truncate text-[12px] text-fg-3">
                    {l.email}
                    {l.phone ? ` · ${l.phone}` : ""}
                  </div>
                  {l.last_message && (
                    <p className="mt-1 line-clamp-2 text-[12px] text-fg-2">
                      {l.last_message}
                    </p>
                  )}
                </div>
                <Badge tone={l.status === "applied" ? "good" : "neutral"}>
                  {app ? `Applied · ${app.status}` : leadStage(l)}
                </Badge>
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                {can("maintenance:manage") && (
                  <Button size="sm" onClick={() => setBooking(l)}>
                    <CalendarPlus />
                    Book a showing
                  </Button>
                )}
                {phone && (
                  <IconLink href={phone.tel} icon={<Phone />} label="Call" />
                )}
                {phone && (
                  <IconLink
                    href={phone.sms}
                    icon={<MessageSquare />}
                    label="Text"
                  />
                )}
                {!l.application_id && can("application:write") && (
                  <InviteButton lead={l} onDone={refresh} />
                )}
                {app?.status === "Approved" && can("lease:manage") && (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => setLeasing({ lead: l, application: app })}
                  >
                    <FileSignature />
                    Write the lease
                  </Button>
                )}
              </div>
            </Panel>
          );
        })}
      </Section>

      {booking && (
        <BookDialog
          lead={booking}
          properties={properties.data ?? []}
          onClose={() => setBooking(null)}
          onBooked={() => {
            setBooking(null);
            refresh();
          }}
        />
      )}
      {adding && (
        <NewLeadDialog
          onClose={() => setAdding(false)}
          onAdded={(l) => {
            setAdding(false);
            refresh();
            setBooking(l);
          }}
        />
      )}
      {leasing && (
        <LeaseDialog
          lead={leasing.lead}
          application={leasing.application}
          propertyId={leasing.property_id}
          properties={properties.data ?? []}
          onClose={() => setLeasing(null)}
          onDone={refresh}
        />
      )}
    </div>
  );
}

function Section({
  title,
  count,
  children,
}: {
  title: string;
  count: number;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <h2 className="flex items-baseline gap-2 text-[13px] font-medium text-fg-2">
        {title}
        {count > 0 && <span className="figure text-fg-4">{count}</span>}
      </h2>
      {children}
    </section>
  );
}

function IconLink({
  href,
  icon,
  label,
}: {
  href: string;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <a
      href={href}
      target={href.startsWith("http") ? "_blank" : undefined}
      rel="noreferrer"
      className="inline-flex h-8 items-center gap-1 rounded-lg border border-line px-2.5 text-[12px] text-fg-2 hover:bg-fill [&>svg]:size-3.5"
    >
      {icon}
      {label}
    </a>
  );
}

function ShowingCard({
  a,
  lead,
  app,
  onChange,
  onLease,
}: {
  a: Appointment;
  lead?: Lead;
  app?: Application;
  onChange: () => void;
  onLease: (lead: Lead, application: Application) => void;
}) {
  const { can } = useAuth();
  const [busy, setBusy] = useState(false);
  const phone = phoneLinks(a.with_phone);
  const address = a.property_name ?? "";
  async function outcome(status: "done" | "no_show") {
    setBusy(true);
    try {
      await appointments.update(a.id, { status });
      toast.success(status === "done" ? "Showing done" : "Marked a no-show");
      onChange();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    } finally {
      setBusy(false);
    }
  }
  return (
    <Panel className={cn("p-4", a.status === "confirmed" && "border-good/30")}>
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <Badge
              tone={
                a.status === "confirmed"
                  ? "good"
                  : a.status === "declined"
                    ? "warn"
                    : "neutral"
              }
            >
              {a.when_words ??
                (a.status === "declined"
                  ? "Needs a new time"
                  : `${a.windows.length} time${a.windows.length === 1 ? "" : "s"} offered`)}
            </Badge>
            {app && (
              <Badge tone={app.status === "Approved" ? "good" : "info"}>
                Applied · {app.status}
              </Badge>
            )}
          </div>
          <div className="mt-1 text-[15px] font-semibold text-fg">
            {a.with_name ?? "Prospect"}
          </div>
          <div className="flex items-center gap-1 truncate text-[13px] text-fg-3">
            <MapPin className="size-3.5 shrink-0" />
            {address}
          </div>
          {a.note && <p className="mt-1 text-[12px] text-fg-2">{a.note}</p>}
        </div>
      </div>
      <div className="mt-3 flex flex-wrap gap-2">
        {phone && <IconLink href={phone.tel} icon={<Phone />} label="Call" />}
        {phone && (
          <IconLink href={phone.sms} icon={<MessageSquare />} label="Text" />
        )}
        {a.with_email && (
          <IconLink
            href={`mailto:${a.with_email}`}
            icon={<Mail />}
            label="Email"
          />
        )}
        <IconLink href={mapLink(address)} icon={<MapPin />} label="Map" />
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-line pt-3">
        {a.status === "confirmed" && can("maintenance:manage") && (
          <>
            <Button size="sm" disabled={busy} onClick={() => outcome("done")}>
              <Check />
              Showed it
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => outcome("no_show")}
            >
              No-show
            </Button>
          </>
        )}
        {lead && !lead.application_id && can("application:write") && (
          <InviteButton lead={lead} onDone={onChange} />
        )}
        {lead && app?.status === "Approved" && can("lease:manage") && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onLease(lead, app)}
          >
            <FileSignature />
            Write the lease
          </Button>
        )}
      </div>
    </Panel>
  );
}

/** Send the application link by email and text; the link is shown too. */
function InviteButton({ lead, onDone }: { lead: Lead; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState<string | null>(null);
  async function go() {
    setBusy(true);
    try {
      const r = await showings.invite(lead.id, {});
      setSent(r.apply_url);
      toast.success(`Application sent to ${lead.name}`);
      onDone();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send it");
    } finally {
      setBusy(false);
    }
  }
  if (sent)
    return (
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          navigator.clipboard?.writeText(sent);
          toast.success("Link copied");
        }}
      >
        <Copy />
        Copy the link
      </Button>
    );
  return (
    <Button size="sm" variant="secondary" disabled={busy} onClick={go}>
      <Send />
      {busy ? "Sending…" : "Send application"}
    </Button>
  );
}

function BookDialog({
  lead,
  properties,
  onClose,
  onBooked,
}: {
  lead: Lead;
  properties: Property[];
  onClose: () => void;
  onBooked: () => void;
}) {
  const [propertyId, setPropertyId] = useState(properties[0]?.id ?? "");
  const [date, setDate] = useState(ymd(new Date()));
  const [time, setTime] = useState("17:00");
  const [date2, setDate2] = useState("");
  const [time2, setTime2] = useState("10:00");
  const [confirmNow, setConfirmNow] = useState(true);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  async function book() {
    const start = instantFrom(date, time);
    if (!propertyId || !start) return;
    setBusy(true);
    try {
      const windows = [{ start }];
      const second = instantFrom(date2, time2);
      if (second && !confirmNow) windows.push({ start: second });
      const made = await appointments.offer({
        property_id: propertyId,
        lead_id: lead.id,
        kind: "showing",
        windows,
        with_role: "prospect",
        note: note.trim() || undefined,
      });
      if (confirmNow)
        await appointments.update(made.id, { confirm: { start } });
      toast.success(
        confirmNow
          ? "Showing booked; they've been told"
          : "Times sent; they pick one"
      );
      onBooked();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't book it");
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Showing for {lead.name}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          They get the time by email and text, with a link to confirm or ask for
          another.
        </DialogDescription>
        <div className="mt-4 space-y-3">
          <select
            aria-label="Property"
            className={cn(field, "w-full")}
            value={propertyId}
            onChange={(e) => setPropertyId(e.target.value)}
          >
            <option value="">Which property?</option>
            {properties.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} · {p.address}
              </option>
            ))}
          </select>
          <div className="flex flex-wrap gap-2">
            <input
              type="date"
              aria-label="Date"
              className={field}
              value={date}
              onChange={(e) => setDate(e.target.value)}
            />
            <input
              type="time"
              aria-label="Time"
              className={field}
              value={time}
              onChange={(e) => setTime(e.target.value)}
            />
          </div>
          <label className="flex items-center gap-2 text-[13px] text-fg-2">
            <input
              type="checkbox"
              checked={confirmNow}
              onChange={(e) => setConfirmNow(e.target.checked)}
              className="size-4 accent-[var(--accent)]"
            />
            We already agreed on this time; book it
          </label>
          {!confirmNow && (
            <div className="flex flex-wrap gap-2">
              <input
                type="date"
                aria-label="Second date"
                className={field}
                value={date2}
                onChange={(e) => setDate2(e.target.value)}
              />
              <input
                type="time"
                aria-label="Second time"
                className={field}
                value={time2}
                onChange={(e) => setTime2(e.target.value)}
              />
              <span className="self-center text-[12px] text-fg-4">
                a second option
              </span>
            </div>
          )}
          <textarea
            className={cn(field, "min-h-[56px] w-full")}
            placeholder="Where to meet, parking (optional)"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button disabled={busy || !propertyId} onClick={book}>
            <CalendarPlus />
            {busy ? "Booking…" : confirmNow ? "Book it" : "Send the times"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function NewLeadDialog({
  onClose,
  onAdded,
}: {
  onClose: () => void;
  onAdded: (l: Lead) => void;
}) {
  const [name, setName] = useState("");
  const [phone, setPhone] = useState("");
  const [email, setEmail] = useState("");
  const [busy, setBusy] = useState(false);
  async function add() {
    setBusy(true);
    try {
      const l = await api.createLead({
        name: name.trim(),
        email: email.trim(),
        phone: phone.trim() || undefined,
        source: "walk_in",
      });
      onAdded(l);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't add them");
      setBusy(false);
    }
  }
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-sm">
        <DialogTitle className="text-[17px] font-semibold">
          New lead
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Someone who called or walked up. Book their showing next.
        </DialogDescription>
        <div className="mt-4 space-y-2">
          <input
            className={cn(field, "w-full")}
            placeholder="Name"
            aria-label="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
          <input
            className={cn(field, "w-full")}
            placeholder="Phone"
            aria-label="Phone"
            inputMode="tel"
            value={phone}
            onChange={(e) => setPhone(e.target.value)}
          />
          <input
            className={cn(field, "w-full")}
            placeholder="Email"
            aria-label="Email"
            inputMode="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            disabled={busy || !name.trim() || !email.trim()}
            onClick={add}
          >
            <Plus />
            Add
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** Approved application → lease → document → sent to sign, in one go. */
function LeaseDialog({
  lead,
  application,
  propertyId,
  properties,
  onClose,
  onDone,
}: {
  lead: Lead;
  application: Application;
  propertyId?: string;
  properties: Property[];
  onClose: () => void;
  onDone: () => void;
}) {
  const [pid, setPid] = useState(propertyId ?? properties[0]?.id ?? "");
  const [rent, setRent] = useState("");
  const [deposit, setDeposit] = useState("");
  const [start, setStart] = useState(application.move_in || ymd(new Date()));
  const [months, setMonths] = useState("12");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [links, setLinks] = useState<EsignSignerLink[] | null>(null);
  const rentCents = dollarsToCents(rent);
  const depositCents = deposit ? dollarsToCents(deposit) : 0;

  async function go() {
    if (!pid || !rentCents) return;
    try {
      setBusy("Writing the lease…");
      const end = new Date(start);
      end.setMonth(end.getMonth() + Math.max(1, Number(months) || 12));
      const lease = await api.convertApplication(application.id, {
        property_id: pid,
        rent_cents: rentCents,
        deposit_cents: depositCents ?? undefined,
        start_date: start,
        end_date: ymd(end),
      });
      setBusy("Building the document…");
      await api.generateLeaseDoc(lease.id);
      setBusy("Sending to sign…");
      const env = await api.createEnvelope(lease.id, {
        message: message.trim() || undefined,
      });
      setLinks(env.sign_links);
      toast.success(`Lease sent to ${lead.name} to sign`);
      onDone();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't send the lease");
    } finally {
      setBusy(null);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Lease for {lead.name}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          Their approved application becomes a lease, the document is written
          from your template, and they get a signing link by email and text.
        </DialogDescription>
        {links ? (
          <div className="mt-4 space-y-2">
            {links.map((l) => {
              const sms = phoneLinks(application.phone);
              return (
                <div
                  key={l.signer_id}
                  className="rounded-xl border border-line p-3"
                >
                  <div className="text-[13px] font-medium text-fg">
                    {l.name}
                  </div>
                  <div className="truncate text-[11px] text-fg-4">
                    {l.sign_url}
                  </div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => {
                        navigator.clipboard?.writeText(l.sign_url);
                        toast.success("Link copied");
                      }}
                    >
                      <Copy />
                      Copy
                    </Button>
                    {sms && l.email === application.email && (
                      <IconLink
                        href={`${sms.sms}?body=${encodeURIComponent(`Here's your lease to sign: ${l.sign_url}`)}`}
                        icon={<MessageSquare />}
                        label="Text it"
                      />
                    )}
                  </div>
                </div>
              );
            })}
            <div className="flex justify-end">
              <Button onClick={onClose}>Done</Button>
            </div>
          </div>
        ) : (
          <>
            <div className="mt-4 space-y-2">
              <select
                aria-label="Property"
                className={cn(field, "w-full")}
                value={pid}
                onChange={(e) => setPid(e.target.value)}
              >
                <option value="">Which property?</option>
                {properties.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name} · {p.address}
                  </option>
                ))}
              </select>
              <div className="grid grid-cols-2 gap-2">
                <input
                  className={field}
                  inputMode="decimal"
                  placeholder="Rent per month"
                  aria-label="Rent"
                  value={rent}
                  onChange={(e) => setRent(e.target.value)}
                />
                <input
                  className={field}
                  inputMode="decimal"
                  placeholder="Deposit"
                  aria-label="Deposit"
                  value={deposit}
                  onChange={(e) => setDeposit(e.target.value)}
                />
                <input
                  className={field}
                  type="date"
                  aria-label="Start date"
                  value={start}
                  onChange={(e) => setStart(e.target.value)}
                />
                <select
                  className={field}
                  aria-label="Term"
                  value={months}
                  onChange={(e) => setMonths(e.target.value)}
                >
                  {["6", "12", "18", "24"].map((m) => (
                    <option key={m} value={m}>
                      {m} months
                    </option>
                  ))}
                </select>
              </div>
              <textarea
                className={cn(field, "min-h-[56px] w-full")}
                placeholder="A note on the signing email (optional)"
                value={message}
                onChange={(e) => setMessage(e.target.value)}
              />
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <Button variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button disabled={!!busy || !pid || !rentCents} onClick={go}>
                <FileSignature />
                {busy ?? "Send the lease to sign"}
              </Button>
            </div>
          </>
        )}
        {!links && !busy && (
          <p className="mt-3 flex items-center gap-1.5 text-[11px] text-fg-4">
            <DoorOpen className="size-3.5" />
            You sign too: your own link comes to your email.
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
