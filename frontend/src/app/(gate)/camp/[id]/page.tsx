"use client";

// The guest's booking page for a campground or RV park: pick dates, see
// which sites are free, pick one, see the price, and send a request. Staff
// confirm it; the guest gets a link to their stay.

import { Suspense, useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { CheckCircle2, Tent } from "lucide-react";
import { DEFAULT_TENANT } from "@/lib/api";
import { addDays, publicCamp, todayIso } from "@/lib/campground";
import { usd } from "@/lib/format";
import { Button } from "@/components/ui/button";
import { fieldClass } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

function errMsg(e: unknown) {
  return e instanceof Error ? e.message : "Something went wrong";
}

export default function CampBookingPage() {
  return (
    <Suspense fallback={null}>
      <Booking />
    </Suspense>
  );
}

function Booking() {
  const { id } = useParams<{ id: string }>();
  const params = useSearchParams();
  const tenant = params.get("tenant") ?? DEFAULT_TENANT;
  const [checkIn, setCheckIn] = useState(
    params.get("in") ?? addDays(todayIso(), 1)
  );
  const [checkOut, setCheckOut] = useState(
    params.get("out") ?? addDays(todayIso(), 3)
  );
  const [site, setSite] = useState("");
  const [addons, setAddons] = useState<Record<string, number>>({});
  const [guest, setGuest] = useState({
    guest_name: "",
    email: "",
    phone: "",
    guests: "2",
    vehicle: "",
    rig: "",
    note: "",
    website: "",
  });
  const datesOk = checkOut > checkIn && checkIn >= todayIso();
  const camp = useQuery({
    queryKey: ["public-camp", id, tenant, checkIn, checkOut, datesOk],
    queryFn: () =>
      datesOk
        ? publicCamp.view(id, tenant, checkIn, checkOut)
        : publicCamp.view(id, tenant),
    retry: false,
  });
  const input = {
    site_id: site,
    check_in: checkIn,
    check_out: checkOut,
    addons: Object.entries(addons).filter(([, n]) => n > 0) as [
      string,
      number,
    ][],
  };
  const chosen = camp.data?.sites.find((s) => s.site.id === site);
  const quote = useQuery({
    queryKey: ["public-camp-quote", id, tenant, input],
    queryFn: () => publicCamp.quote(id, input, tenant),
    enabled: !!site && datesOk && chosen?.free !== false,
    retry: false,
  });
  const send = useMutation({
    mutationFn: () =>
      publicCamp.request(
        id,
        {
          ...input,
          guest_name: guest.guest_name.trim(),
          email: guest.email.trim(),
          phone: guest.phone.trim() || undefined,
          guests: Number(guest.guests) || 1,
          vehicle: guest.vehicle.trim() || undefined,
          rig_length_ft: guest.rig ? Number(guest.rig) : undefined,
          note: guest.note.trim() || undefined,
          website: guest.website,
        },
        tenant
      ),
  });

  if (camp.isLoading)
    return (
      <Shell>
        <Skeleton className="h-64 rounded-2xl" />
      </Shell>
    );
  if (camp.error || !camp.data)
    return (
      <Shell>
        <Panel className="p-6 text-center text-[14px] text-fg-2">
          This campground isn&apos;t taking bookings online right now.
        </Panel>
      </Shell>
    );
  const c = camp.data;

  if (send.data)
    return (
      <Shell>
        <Panel className="space-y-3 p-6 text-center">
          <CheckCircle2 className="mx-auto size-10 text-good" />
          <h1 className="text-[20px] font-semibold text-fg">Request sent</h1>
          <p className="text-[14px] text-fg-2">
            {c.company} will confirm {send.data.stay.site_name} for{" "}
            {send.data.stay.check_in} to {send.data.stay.check_out}. We emailed
            you a link to your stay.
          </p>
          <a
            href={send.data.link}
            className="inline-block text-[14px] font-medium text-accent hover:underline"
          >
            See your stay
          </a>
        </Panel>
      </Shell>
    );

  const f = (
    k: keyof typeof guest,
    label: string,
    type = "text",
    required = false
  ) => (
    <label className="text-[12px] text-fg-3">
      {label}
      <input
        type={type}
        required={required}
        className={cn(fieldClass, "mt-1 w-full")}
        value={guest[k]}
        onChange={(e) => setGuest({ ...guest, [k]: e.target.value })}
      />
    </label>
  );
  const ready =
    !!quote.data && guest.guest_name.trim() && guest.email.includes("@");

  return (
    <Shell>
      <div className="space-y-1">
        <div className="eyebrow">{c.company}</div>
        <h1 className="text-[24px] font-semibold text-fg">{c.name}</h1>
        <p className="text-[13px] text-fg-3">
          Check in after {c.check_in_time}, check out by {c.check_out_time}.
          Stays up to {c.max_nights} nights.
        </p>
      </div>

      <Panel className="grid gap-3 p-5 sm:grid-cols-2">
        <label className="text-[12px] text-fg-3">
          Arrive
          <input
            type="date"
            min={todayIso()}
            className={cn(fieldClass, "mt-1 w-full")}
            value={checkIn}
            onChange={(e) => setCheckIn(e.target.value)}
          />
        </label>
        <label className="text-[12px] text-fg-3">
          Leave
          <input
            type="date"
            min={addDays(checkIn, 1)}
            className={cn(fieldClass, "mt-1 w-full")}
            value={checkOut}
            onChange={(e) => setCheckOut(e.target.value)}
          />
        </label>
        {!datesOk && (
          <p className="text-[13px] text-bad sm:col-span-2">
            Pick an arrival from today on, and a later day to leave.
          </p>
        )}
      </Panel>

      <div className="space-y-2">
        <div className="eyebrow">Sites</div>
        <div className="grid gap-2 sm:grid-cols-2">
          {c.sites.map(({ site: s, free }) => {
            const off = s.closed || free === false;
            return (
              <button
                key={s.id}
                type="button"
                disabled={off}
                onClick={() => setSite(s.id)}
                className={cn(
                  "rounded-xl border px-4 py-3 text-left transition",
                  site === s.id
                    ? "border-accent bg-accent/5"
                    : "border-line bg-surface hover:border-fg-4",
                  off && "cursor-not-allowed opacity-50"
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="flex items-center gap-1.5 text-[14px] font-semibold text-fg">
                    <Tent className="size-4 text-fg-3" />
                    {s.name}
                  </span>
                  <span className="text-[12px] text-fg-3">
                    {off
                      ? "Taken"
                      : s.rate_cents_night
                        ? `${usd(s.rate_cents_night)}/night`
                        : ""}
                  </span>
                </div>
                <div className="text-[12px] text-fg-3">
                  {[
                    s.site_type,
                    s.power_amps && `${s.power_amps} amp`,
                    s.water && "water",
                    s.sewer && "sewer",
                    s.pull_through && "pull-through",
                    s.pets && "pets ok",
                    s.max_length_ft && `up to ${s.max_length_ft} ft`,
                    s.max_guests && `${s.max_guests} guests`,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
              </button>
            );
          })}
        </div>
      </div>

      {site && (
        <Panel className="space-y-4 p-5">
          {c.addons.length > 0 && (
            <div className="space-y-2">
              <div className="eyebrow">Extras</div>
              {c.addons.map((a) => (
                <label
                  key={a.key}
                  className="flex items-center gap-3 text-[13px] text-fg-2"
                >
                  <input
                    type="number"
                    min={0}
                    max={20}
                    aria-label={a.label}
                    className={cn(fieldClass, "w-20 py-1")}
                    value={addons[a.key] ?? 0}
                    onChange={(e) =>
                      setAddons({ ...addons, [a.key]: Number(e.target.value) })
                    }
                  />
                  {a.label} · {usd(a.price_cents)} per {a.per}
                </label>
              ))}
            </div>
          )}
          {quote.error && (
            <p className="text-[13px] text-bad">{errMsg(quote.error)}</p>
          )}
          {quote.data && (
            <dl className="space-y-1.5 rounded-xl bg-fill p-4 text-[13px]">
              <div className="flex justify-between">
                <dt className="text-fg-3">
                  {quote.data.nights} night{quote.data.nights === 1 ? "" : "s"}
                </dt>
                <dd className="font-mono">{usd(quote.data.stay_cents)}</dd>
              </div>
              {quote.data.addons.map((a) => (
                <div key={a.key} className="flex justify-between">
                  <dt className="text-fg-3">
                    {a.label} × {a.qty}
                  </dt>
                  <dd className="font-mono">{usd(a.cents)}</dd>
                </div>
              ))}
              <div className="flex justify-between border-t border-line pt-1.5 font-semibold text-fg">
                <dt>Total</dt>
                <dd className="font-mono">{usd(quote.data.total_cents)}</dd>
              </div>
              {quote.data.deposit_cents > 0 && (
                <div className="flex justify-between text-fg-3">
                  <dt>Deposit to hold it</dt>
                  <dd className="font-mono">{usd(quote.data.deposit_cents)}</dd>
                </div>
              )}
            </dl>
          )}
          <form
            className="grid gap-3 sm:grid-cols-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (ready) send.mutate();
            }}
          >
            {f("guest_name", "Your name", "text", true)}
            {f("email", "Email", "email", true)}
            {f("phone", "Phone", "tel")}
            {f("guests", "Guests", "number")}
            {f("vehicle", "Vehicle or rig")}
            {f("rig", "Rig length (ft)", "number")}
            <label className="text-[12px] text-fg-3 sm:col-span-2">
              Anything we should know
              <textarea
                rows={2}
                className={cn(fieldClass, "mt-1 w-full")}
                value={guest.note}
                onChange={(e) => setGuest({ ...guest, note: e.target.value })}
              />
            </label>
            <input
              type="text"
              name="website"
              tabIndex={-1}
              autoComplete="off"
              aria-hidden
              className="hidden"
              value={guest.website}
              onChange={(e) => setGuest({ ...guest, website: e.target.value })}
            />
            {c.policies && (
              <p className="text-[12px] whitespace-pre-line text-fg-3 sm:col-span-2">
                {c.policies}
              </p>
            )}
            {send.error && (
              <p className="text-[13px] text-bad sm:col-span-2">
                {errMsg(send.error)}
              </p>
            )}
            <Button
              type="submit"
              className="sm:col-span-2"
              disabled={!ready}
              loading={send.isPending}
            >
              Request this site
            </Button>
          </form>
        </Panel>
      )}
    </Shell>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-dvh bg-bg">
      <main className="mx-auto max-w-2xl space-y-5 px-4 py-8">{children}</main>
    </div>
  );
}
