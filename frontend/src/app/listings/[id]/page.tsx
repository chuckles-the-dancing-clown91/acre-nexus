"use client";

// Public listing detail + rental application. The public website is white-label:
// on mount we apply the default tenant's brand so the SiteHeader + accent tokens
// reflect their identity. Renders a gradient hero, the listing facts, and an
// "Apply" card driving the rental application (react-hook-form + zod →
// api.apply). On success we swap the form for a confirmation state and toast.

import { useEffect } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useQuery, useMutation } from "@tanstack/react-query";
import {
  ArrowLeft,
  BedDouble,
  Bath,
  Ruler,
  CalendarCheck,
  CheckCircle2,
  Home,
  Loader2,
} from "lucide-react";
import { toast } from "sonner";

import { api, DEFAULT_TENANT } from "@/lib/api";
import type { ApplyResponse, Listing } from "@/lib/types";
import { useTheme } from "@/lib/theme";
import { gradFor } from "@/lib/gradients";

import SiteHeader from "@/components/SiteHeader";
import { Badge, statusTone } from "@/components/ui";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/page";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/form-field";

export default function ListingDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params.id;
  const { setBrandTenant } = useTheme();

  // Public site is white-label: apply the demo tenant's brand on mount.
  useEffect(() => {
    void setBrandTenant(DEFAULT_TENANT);
  }, [setBrandTenant]);

  const listingQuery = useQuery<Listing>({
    queryKey: ["public-listing", id],
    queryFn: () => api.publicListing(id),
    enabled: !!id,
    retry: false,
  });

  return (
    <>
      <SiteHeader />
      <main className="mx-auto max-w-[1100px] px-6 py-8">
        <Link
          href="/"
          className="mb-5 inline-flex items-center gap-2 text-sm font-semibold text-ink-2 transition hover:text-ink"
        >
          <ArrowLeft className="h-4 w-4" /> All listings
        </Link>

        {listingQuery.isLoading ? (
          <ListingSkeleton />
        ) : listingQuery.isError || !listingQuery.data ? (
          <EmptyState
            icon={Home}
            title="Listing not found"
            description="This listing may have been removed or is no longer available."
            action={
              <Button asChild>
                <Link href="/">Browse all listings</Link>
              </Button>
            }
          />
        ) : (
          <ListingDetail listing={listingQuery.data} />
        )}
      </main>
    </>
  );
}

function ListingDetail({ listing }: { listing: Listing }) {
  return (
    <div className="grid gap-7 md:grid-cols-[1.6fr_1fr]">
      <div>
        {/* Gradient hero */}
        <div
          className="relative mb-4 aspect-video rounded-[20px] shadow-acre-lg"
          style={{ background: gradFor(0) }}
        >
          <div className="absolute left-4 top-4">
            <Badge tone={statusTone(listing.status)}>{listing.status}</Badge>
          </div>
        </div>

        <h1 className="mb-1 font-display text-3xl font-extrabold tracking-tight text-ink">
          {listing.title}
        </h1>
        <p className="mb-5 text-ink-3">
          {listing.address} · {listing.city}
        </p>

        <div className="mb-6 flex flex-wrap gap-x-6 gap-y-2 text-sm font-semibold text-ink-2">
          <span className="inline-flex items-center gap-1.5">
            <BedDouble className="h-4 w-4 text-ink-3" />
            {listing.beds === 0 ? "Studio" : `${listing.beds} beds`}
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Bath className="h-4 w-4 text-ink-3" />
            {listing.baths} baths
          </span>
          <span className="inline-flex items-center gap-1.5" data-numeric>
            <Ruler className="h-4 w-4 text-ink-3" />
            {listing.sqft.toLocaleString()} sqft
          </span>
          <span className="inline-flex items-center gap-1.5">
            <CalendarCheck className="h-4 w-4 text-ink-3" />
            Available {listing.available_on}
          </span>
        </div>

        <p className="leading-relaxed text-ink-2">{listing.description}</p>
      </div>

      {/* Rent + apply card */}
      <div>
        <Card className="sticky top-20">
          <CardHeader className="block border-b-0 pb-0">
            <div className="font-display text-3xl font-extrabold text-ink">
              {listing.rent_label}
              <span className="text-base font-semibold text-ink-3">/mo</span>
            </div>
            <p className="mt-1 text-sm text-ink-3">
              Apply once — screening runs automatically.
            </p>
          </CardHeader>
          <CardContent>
            <ApplyForm listingId={listing.id} />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

// ---- Rental application form ------------------------------------------------

const applySchema = z.object({
  applicant_name: z.string().min(1, "Required"),
  email: z.string().min(1, "Required").email("Enter a valid email"),
  phone: z.string().optional(),
  income: z.coerce
    .number({ invalid_type_error: "Enter a number" })
    .nonnegative("Must be 0 or more")
    .optional(),
  move_in: z.string().optional(),
});

type ApplyFormValues = z.input<typeof applySchema>;

function ApplyForm({ listingId }: { listingId: string }) {
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<ApplyFormValues>({
    resolver: zodResolver(applySchema),
    defaultValues: {
      applicant_name: "",
      email: "",
      phone: "",
      income: undefined,
      move_in: "",
    },
  });

  const apply = useMutation<ApplyResponse, Error, ApplyFormValues>({
    mutationFn: (values) =>
      api.apply({
        listing_id: listingId,
        applicant_name: values.applicant_name,
        email: values.email,
        phone: values.phone ?? "",
        annual_income_cents: values.income
          ? Math.round(Number(values.income) * 100)
          : 0,
        move_in: values.move_in ?? "",
      }),
    onSuccess: () => toast.success("Application received"),
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Submission failed"),
  });

  const onSubmit = handleSubmit((values) => apply.mutate(values));

  if (apply.isSuccess && apply.data) {
    const res = apply.data;
    return (
      <div className="rounded-xl border border-good-soft bg-good-soft p-4 text-good">
        <div className="mb-1 flex items-center gap-2 font-bold">
          <CheckCircle2 className="h-[18px] w-[18px]" /> Application received
        </div>
        <p className="text-sm">{res.message}</p>
        <p className="mt-2 font-mono text-xs opacity-80">
          Screening job: {res.screening_job_id.slice(0, 8)}…
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={onSubmit} className="space-y-3">
      <Field label="Full name" htmlFor="applicant_name" error={errors.applicant_name?.message} required>
        <Input
          id="applicant_name"
          placeholder="Jane Doe"
          autoComplete="name"
          error={!!errors.applicant_name}
          {...register("applicant_name")}
        />
      </Field>
      <Field label="Email" htmlFor="email" error={errors.email?.message} required>
        <Input
          id="email"
          type="email"
          placeholder="jane@example.com"
          autoComplete="email"
          error={!!errors.email}
          {...register("email")}
        />
      </Field>
      <Field label="Phone" htmlFor="phone" error={errors.phone?.message}>
        <Input
          id="phone"
          type="tel"
          placeholder="(555) 555-5555"
          autoComplete="tel"
          error={!!errors.phone}
          {...register("phone")}
        />
      </Field>
      <Field label="Annual income (USD)" htmlFor="income" error={errors.income?.message}>
        <Input
          id="income"
          type="number"
          min="0"
          step="1000"
          placeholder="65000"
          error={!!errors.income}
          {...register("income")}
        />
      </Field>
      <Field label="Desired move-in" htmlFor="move_in" error={errors.move_in?.message}>
        <Input
          id="move_in"
          placeholder="e.g. Aug 1"
          error={!!errors.move_in}
          {...register("move_in")}
        />
      </Field>

      <Button type="submit" disabled={apply.isPending} className="w-full" size="lg">
        {apply.isPending ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          "Apply now"
        )}
      </Button>
    </form>
  );
}

// ---- Loading skeleton -------------------------------------------------------

function ListingSkeleton() {
  return (
    <div className="grid gap-7 md:grid-cols-[1.6fr_1fr]">
      <div>
        <div className="skeleton mb-4 aspect-video rounded-[20px]" />
        <div className="skeleton mb-2 h-8 w-2/3 rounded-lg" />
        <div className="skeleton mb-5 h-4 w-1/2 rounded-lg" />
        <div className="mb-6 flex gap-6">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton h-4 w-20 rounded-lg" />
          ))}
        </div>
        <div className="space-y-2">
          <div className="skeleton h-4 w-full rounded-lg" />
          <div className="skeleton h-4 w-full rounded-lg" />
          <div className="skeleton h-4 w-3/4 rounded-lg" />
        </div>
      </div>
      <div>
        <Card className="sticky top-20 p-5">
          <div className="skeleton mb-2 h-9 w-32 rounded-lg" />
          <div className="skeleton mb-5 h-4 w-full rounded-lg" />
          <div className="space-y-3">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="skeleton h-11 rounded-lg" />
            ))}
            <div className="skeleton h-11 rounded-lg" />
          </div>
        </Card>
      </div>
    </div>
  );
}
