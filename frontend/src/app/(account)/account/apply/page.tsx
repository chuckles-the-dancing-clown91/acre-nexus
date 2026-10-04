"use client";

// "Apply now": the application fills itself in from the resident's profile.
// They check it, pick a move-in date, authorize screening and send.

import { Suspense, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, CircleAlert, ClipboardList } from "lucide-react";
import { api, ApiError, DEFAULT_TENANT } from "@/lib/api";
import { parseIncomeCents } from "@/lib/portal-format";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2.5 text-[14px] text-fg outline-none focus:border-accent";

export default function ApplyPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64" />}>
      <Apply />
    </Suspense>
  );
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  const empty = value == null || value === "";
  return (
    <div className="flex items-baseline justify-between gap-4 py-1.5 text-[13px]">
      <dt className="text-fg-3">{label}</dt>
      <dd className={empty ? "text-fg-4" : "text-right text-fg"}>
        {empty ? "Not filled in" : value}
      </dd>
    </div>
  );
}

function Apply() {
  const qc = useQueryClient();
  const params = useSearchParams();
  const listingId = params.get("listing") ?? "";
  const tenant = params.get("tenant") ?? DEFAULT_TENANT;
  const resident = useQuery({
    queryKey: ["my-resident"],
    queryFn: api.myResident,
    retry: (n, e) => !(e instanceof ApiError && e.status < 500) && n < 2,
  });
  const listing = useQuery({
    queryKey: ["apply-listing", tenant, listingId],
    queryFn: () => api.publicListing(listingId, tenant),
    enabled: !!listingId,
    retry: false,
  });
  const [moveIn, setMoveIn] = useState("");
  const [income, setIncome] = useState<string | null>(null);
  const [consent, setConsent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);

  const p = resident.data?.prefill;
  const incomeText =
    income ??
    (p?.annual_income_cents != null ? String(p.annual_income_cents / 100) : "");

  async function send(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const cents = parseIncomeCents(incomeText);
    if (cents === undefined) {
      setError("Income should be a number, like 52000.");
      return;
    }
    if (!consent) {
      setError("Please authorize the background check to continue.");
      return;
    }
    setBusy(true);
    try {
      await api.myApply({
        listing_id: listingId || undefined,
        move_in: moveIn || undefined,
        annual_income_cents: cents ?? undefined,
        screening_consent: true,
      });
      qc.invalidateQueries({ queryKey: ["my-applications"] });
      setSent(true);
    } catch (e) {
      setError(
        e instanceof Error ? e.message : "Couldn't send your application"
      );
    } finally {
      setBusy(false);
    }
  }

  if (sent) {
    return (
      <Panel className="mx-auto max-w-md space-y-3 p-6 text-center">
        <span className="mx-auto flex size-12 items-center justify-center rounded-full bg-good/15 text-good">
          <Check className="size-6" />
        </span>
        <h1 className="text-[20px] font-semibold text-fg">Application sent</h1>
        <p className="text-[13px] text-fg-3">
          Screening runs on its own and we email you at each step.
        </p>
        <Button asChild>
          <Link href="/account/applications">See my applications</Link>
        </Button>
      </Panel>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Apply now</h1>
        <p className="text-[13px] text-fg-3">
          We filled this in from your profile. Check it, then send.
        </p>
      </div>

      {resident.isLoading && <Skeleton className="h-64" />}
      {resident.error && (
        <Panel className="p-4 text-[13px] text-bad">
          Couldn&apos;t load your profile: {resident.error.message}
        </Panel>
      )}

      {listing.data && (
        <Panel className="flex items-center gap-3 p-4">
          <ClipboardList className="size-5 shrink-0 text-accent" />
          <div className="min-w-0">
            <div className="truncate text-[14px] font-semibold text-fg">
              {listing.data.title}
            </div>
            <div className="truncate text-xs text-fg-3">
              {listing.data.address}, {listing.data.city} ·{" "}
              {listing.data.rent_label}/mo
            </div>
          </div>
        </Panel>
      )}

      {p && (
        <form onSubmit={send} className="space-y-6">
          <Panel>
            <PanelHeader
              title="From your profile"
              description="Change any of this on your profile."
              action={
                <Button asChild size="sm" variant="secondary">
                  <Link href="/account/profile">Edit profile</Link>
                </Button>
              }
            />
            <dl className="divide-y divide-line px-5 pt-2 pb-4">
              <Row label="Name" value={p.name} />
              <Row label="Email" value={p.email} />
              <Row label="Phone" value={p.phone} />
              <Row label="Current address" value={p.current_address} />
              <Row
                label="Work"
                value={
                  p.employer
                    ? `${p.employer}${p.job_title ? `, ${p.job_title}` : ""}`
                    : null
                }
              />
              <Row label="Emergency contact" value={p.emergency_contact} />
              <Row
                label="Pets"
                value={
                  p.pets.length > 0
                    ? p.pets.map((x) => `${x.name} (${x.kind})`).join(", ")
                    : "None"
                }
              />
              <Row
                label="Others living with you"
                value={p.occupants > 0 ? String(p.occupants) : "Just you"}
              />
              <Row
                label="Vehicles"
                value={p.vehicles > 0 ? String(p.vehicles) : "None"}
              />
              <Row
                label="Rental history"
                value={
                  p.prior_rentals > 0
                    ? `${p.prior_rentals} ${p.prior_rentals === 1 ? "place" : "places"}`
                    : null
                }
              />
            </dl>
            {p.missing.length > 0 && (
              <div className="mx-5 mb-5 flex items-start gap-2 rounded-xl border border-warn/30 bg-warn/10 p-3 text-[13px] text-fg-2">
                <CircleAlert className="mt-0.5 size-4 shrink-0 text-warn" />
                <div className="space-x-1">
                  <span>A fuller profile helps. Still empty:</span>
                  {p.missing.map((m) => (
                    <Badge key={m} tone="warn">
                      {m}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
          </Panel>

          <Panel>
            <PanelHeader title="This application" />
            <div className="grid gap-3 px-5 pt-3 pb-5 sm:grid-cols-2">
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-fg-2">
                  Move-in date
                </span>
                <input
                  type="date"
                  className={field}
                  value={moveIn}
                  onChange={(e) => setMoveIn(e.target.value)}
                />
              </label>
              <label className="block">
                <span className="mb-1 block text-xs font-medium text-fg-2">
                  Yearly income ($)
                </span>
                <input
                  className={field}
                  inputMode="decimal"
                  value={incomeText}
                  onChange={(e) => setIncome(e.target.value)}
                  placeholder="52,000"
                />
              </label>
            </div>
            <label className="mx-5 mb-5 flex items-start gap-2 text-[13px] text-fg">
              <input
                type="checkbox"
                className="mt-0.5"
                checked={consent}
                onChange={(e) => setConsent(e.target.checked)}
              />
              <span>
                I authorize a background and credit check for this application.
              </span>
            </label>
          </Panel>

          {error && <p className="text-[13px] text-bad">{error}</p>}
          <div className="flex justify-end">
            <Button type="submit" disabled={busy}>
              {busy ? "Sending..." : "Send application"}
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
