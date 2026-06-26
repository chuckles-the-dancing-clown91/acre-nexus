"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import type { ApplyResponse, Listing } from "@/lib/types";
import { SiteHeader } from "@/components/SiteHeader";
import { Badge, Button, Card, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";
import { gradFor } from "@/lib/gradients";

export default function ListingDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;
  const [listing, setListing] = useState<Listing | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    api
      .publicListing(id)
      .then(setListing)
      .catch((e) => setError(e.message));
  }, [id]);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[1100px] px-6 py-8">
        <Link
          href="/"
          className="mb-5 inline-flex items-center gap-2 text-sm font-semibold text-ink-2"
        >
          <Icon name="back" size={16} /> All listings
        </Link>

        {error && (
          <p className="text-bad">Couldn&apos;t load listing: {error}</p>
        )}

        {listing && (
          <div className="grid gap-7 md:grid-cols-[1.6fr_1fr]">
            <div>
              <div
                className="relative mb-3 aspect-video rounded-[20px] shadow-acre-lg"
                style={{ background: gradFor(0) }}
              >
                <div className="absolute left-4 top-4">
                  <Badge tone={statusTone(listing.status)}>
                    {listing.status}
                  </Badge>
                </div>
              </div>
              <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight">
                {listing.title}
              </h1>
              <p className="mb-5 text-ink-3">
                {listing.address} · {listing.city}
              </p>
              <div className="mb-6 flex gap-6 text-sm font-semibold text-ink-2">
                <span>
                  {listing.beds === 0 ? "Studio" : `${listing.beds} beds`}
                </span>
                <span>{listing.baths} baths</span>
                <span>{listing.sqft.toLocaleString()} sqft</span>
                <span>Available {listing.available_on}</span>
              </div>
              <p className="leading-relaxed text-ink-2">
                {listing.description}
              </p>
            </div>

            <div>
              <Card className="sticky top-20 p-5">
                <div className="font-display text-3xl font-extrabold">
                  {listing.rent_label}
                  <span className="text-base font-semibold text-ink-3">
                    /mo
                  </span>
                </div>
                <p className="mb-4 mt-1 text-sm text-ink-3">
                  Apply once — screening runs automatically.
                </p>
                <ApplyForm listingId={listing.id} />
              </Card>
            </div>
          </div>
        )}
      </main>
    </>
  );
}

function ApplyForm({ listingId }: { listingId: string }) {
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<ApplyResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    applicant_name: "",
    email: "",
    phone: "",
    income: "",
    move_in: "",
  });

  const update =
    (k: keyof typeof form) => (e: React.ChangeEvent<HTMLInputElement>) =>
      setForm((f) => ({ ...f, [k]: e.target.value }));

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const res = await api.apply({
        listing_id: listingId,
        applicant_name: form.applicant_name,
        email: form.email,
        phone: form.phone,
        annual_income_cents: form.income ? Number(form.income) * 100 : 0,
        move_in: form.move_in,
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Submission failed");
    } finally {
      setSubmitting(false);
    }
  }

  if (result) {
    return (
      <div className="rounded-xl border border-good-soft bg-good-soft p-4 text-good">
        <div className="mb-1 flex items-center gap-2 font-bold">
          <Icon name="check" size={18} /> Application received
        </div>
        <p className="text-sm">{result.message}</p>
        <p className="mt-2 font-mono text-xs opacity-80">
          Screening job: {result.screening_job_id.slice(0, 8)}…
        </p>
      </div>
    );
  }

  const field =
    "w-full rounded-xl border border-line bg-surface-2 px-3 py-2.5 text-sm outline-none focus:border-accent";

  return (
    <form onSubmit={submit} className="space-y-2.5">
      <input
        required
        placeholder="Full name"
        className={field}
        value={form.applicant_name}
        onChange={update("applicant_name")}
      />
      <input
        required
        type="email"
        placeholder="Email"
        className={field}
        value={form.email}
        onChange={update("email")}
      />
      <input
        placeholder="Phone"
        className={field}
        value={form.phone}
        onChange={update("phone")}
      />
      <input
        type="number"
        placeholder="Annual income (USD)"
        className={field}
        value={form.income}
        onChange={update("income")}
      />
      <input
        placeholder="Desired move-in (e.g. Aug 1)"
        className={field}
        value={form.move_in}
        onChange={update("move_in")}
      />
      {error && <p className="text-sm text-bad">{error}</p>}
      <Button type="submit" disabled={submitting} className="w-full">
        {submitting ? "Submitting…" : "Apply now"}
      </Button>
    </form>
  );
}
