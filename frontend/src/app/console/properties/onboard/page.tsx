"use client";

// Property onboarding. A clean, multi-section form (react-hook-form + zod)
// grouped into Cards — Property, Financials & strategy, Holding entity, and
// optional Financing (mortgages) — that submits to `api.onboardProperty` and
// routes to the new property profile on success. Gated by "property:write".

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useFieldArray, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useMutation } from "@tanstack/react-query";
import { toast } from "sonner";
import { ArrowLeft, Building2, Plus, ShieldAlert, Trash2 } from "lucide-react";

import { api } from "@/lib/api";
import type { OnboardInput, OnboardMortgageInput } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { useLlcGroups } from "@/lib/queries";
import { titleCase } from "@/lib/format";
import {
  PageHeader,
  EmptyState,
} from "@/components/ui/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { TextField, SelectField } from "@/components/ui/form-field";

// ---- Options ---------------------------------------------------------------
const PROPERTY_TYPES = [
  "single_family",
  "multi_family",
  "condo",
  "townhome",
  "commercial",
  "land",
] as const;

const STRATEGIES = ["rental", "flip", "brrrr", "hold", "wholesale"] as const;

const MORTGAGE_KINDS = [
  "purchase",
  "refinance",
  "heloc",
  "private",
  "hard_money",
  "seller_finance",
] as const;

function humanize(key: string): string {
  return titleCase(key.replace(/_/g, " "));
}

// ---- Schema ----------------------------------------------------------------
// Numeric fields are kept as strings in the form (native number inputs emit
// strings) and coerced to cents / bps / ints when building the API payload.
const mortgageSchema = z.object({
  lender_name: z.string(),
  kind: z.string().min(1),
  original_amount: z.string(),
  current_balance: z.string(),
  interest_rate: z.string(),
  monthly_payment: z.string(),
  escrow: z.string(),
  term_months: z.string(),
  loan_number: z.string(),
  start_date: z.string(),
  maturity_date: z.string(),
});

const schema = z.object({
  name: z.string().trim().min(1, "Name is required"),
  address: z.string().trim().min(1, "Address is required"),
  city: z.string().trim().min(1, "City is required"),
  property_type: z.string().min(1, "Choose a property type"),
  strategy: z.string().min(1, "Choose a strategy"),
  units: z.string(),
  occupied_units: z.string(),
  year_built: z.string(),
  monthly_rent: z.string(),
  purchase_price: z.string(),
  acquired_on: z.string(),
  manager: z.string(),
  llc_id: z.string(),
  enrich: z.boolean(),
  mortgages: z.array(mortgageSchema),
});

type FormValues = z.infer<typeof schema>;
type MortgageValues = z.infer<typeof mortgageSchema>;

function emptyMortgage(): MortgageValues {
  return {
    lender_name: "",
    kind: "purchase",
    original_amount: "",
    current_balance: "",
    interest_rate: "",
    monthly_payment: "",
    escrow: "",
    term_months: "",
    loan_number: "",
    start_date: "",
    maturity_date: "",
  };
}

const DEFAULTS: FormValues = {
  name: "",
  address: "",
  city: "",
  property_type: "",
  strategy: "",
  units: "",
  occupied_units: "",
  year_built: "",
  monthly_rent: "",
  purchase_price: "",
  acquired_on: "",
  manager: "",
  llc_id: "",
  enrich: true,
  mortgages: [],
};

// ---- Conversion helpers ----------------------------------------------------
function dollarsToCents(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n * 100);
}

function percentToBps(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n * 100);
}

function toInt(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n);
}

function toStr(v: string): string | undefined {
  const t = v.trim();
  return t === "" ? undefined : t;
}

function isMortgageEmpty(m: MortgageValues): boolean {
  return (
    m.lender_name.trim() === "" &&
    m.original_amount.trim() === "" &&
    m.current_balance.trim() === "" &&
    m.interest_rate.trim() === "" &&
    m.monthly_payment.trim() === "" &&
    m.escrow.trim() === "" &&
    m.term_months.trim() === "" &&
    m.loan_number.trim() === "" &&
    m.start_date.trim() === "" &&
    m.maturity_date.trim() === ""
  );
}

function toMortgageInput(m: MortgageValues): OnboardMortgageInput {
  const out: OnboardMortgageInput = { kind: m.kind };
  const lender = toStr(m.lender_name);
  if (lender !== undefined) out.lender_name = lender;
  const original = dollarsToCents(m.original_amount);
  if (original !== undefined) out.original_amount_cents = original;
  const balance = dollarsToCents(m.current_balance);
  if (balance !== undefined) out.current_balance_cents = balance;
  const rate = percentToBps(m.interest_rate);
  if (rate !== undefined) out.interest_rate_bps = rate;
  const payment = dollarsToCents(m.monthly_payment);
  if (payment !== undefined) out.monthly_payment_cents = payment;
  const escrow = dollarsToCents(m.escrow);
  if (escrow !== undefined) out.escrow_monthly_cents = escrow;
  const term = toInt(m.term_months);
  if (term !== undefined) out.term_months = term;
  const loanNo = toStr(m.loan_number);
  if (loanNo !== undefined) out.loan_number = loanNo;
  const start = toStr(m.start_date);
  if (start !== undefined) out.start_date = start;
  const maturity = toStr(m.maturity_date);
  if (maturity !== undefined) out.maturity_date = maturity;
  return out;
}

function buildInput(v: FormValues): OnboardInput {
  const input: OnboardInput = {
    name: v.name.trim(),
    address: v.address.trim(),
    city: v.city.trim(),
    property_type: v.property_type,
    strategy: v.strategy,
    mortgages: v.mortgages.filter((m) => !isMortgageEmpty(m)).map(toMortgageInput),
    enrich: v.enrich,
  };
  const units = toInt(v.units);
  if (units !== undefined) input.units = units;
  const occupied = toInt(v.occupied_units);
  if (occupied !== undefined) input.occupied_units = occupied;
  const rent = dollarsToCents(v.monthly_rent);
  if (rent !== undefined) input.monthly_rent_cents = rent;
  const year = toInt(v.year_built);
  if (year !== undefined) input.year_built = year;
  const price = dollarsToCents(v.purchase_price);
  if (price !== undefined) input.purchase_price_cents = price;
  const acquired = toStr(v.acquired_on);
  if (acquired !== undefined) input.acquired_on = acquired;
  const manager = toStr(v.manager);
  if (manager !== undefined) input.manager = manager;
  const llc = toStr(v.llc_id);
  if (llc !== undefined) input.llc_id = llc;
  return input;
}

// ---- Page ------------------------------------------------------------------
export default function OnboardPropertyPage() {
  const { can } = useAuth();
  const router = useRouter();
  const llcs = useLlcGroups();

  const {
    register,
    handleSubmit,
    control,
    setValue,
    watch,
    formState: { errors },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: DEFAULTS,
  });

  const { fields, append, remove } = useFieldArray({
    control,
    name: "mortgages",
  });

  const enrich = watch("enrich");

  const onboard = useMutation({
    mutationFn: (input: OnboardInput) => api.onboardProperty(input),
    onSuccess: (resp) => {
      toast.success("Property onboarded");
      router.push(`/console/properties/${resp.property_id}`);
    },
    onError: (e) =>
      toast.error(
        e instanceof Error ? e.message : "Failed to onboard property."
      ),
  });

  if (!can("property:write")) {
    return (
      <div className="space-y-6">
        <PageHeader
          eyebrow="Properties"
          title="Onboard a property"
          description="Add a property to your managed portfolio."
        />
        <EmptyState
          icon={ShieldAlert}
          title="You don't have access"
          description="Ask a platform admin to grant the property:write permission to onboard properties."
          action={
            <Button asChild variant="outline">
              <Link href="/console/properties">Back to properties</Link>
            </Button>
          }
        />
      </div>
    );
  }

  const onSubmit = handleSubmit((values) => {
    onboard.mutate(buildInput(values));
  });

  const submitting = onboard.isPending;

  return (
    <form onSubmit={onSubmit} className="space-y-6">
      <PageHeader
        eyebrow="Properties"
        title="Onboard a property"
        description="Capture property details, financing, and kick off enrichment."
        actions={
          <Button asChild variant="ghost" size="sm">
            <Link href="/console/properties">
              <ArrowLeft className="h-4 w-4" />
              Back to properties
            </Link>
          </Button>
        }
      />

      {/* Property */}
      <Card>
        <CardHeader className="block border-b-0 pb-0">
          <CardTitle>Property</CardTitle>
          <CardDescription>
            Where it is and what kind of asset it is.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <TextField
            label="Name"
            required
            placeholder="Birchwood Lofts"
            error={errors.name?.message}
            {...register("name")}
          />
          <TextField
            label="Address"
            required
            placeholder="88 Birch Ave"
            error={errors.address?.message}
            {...register("address")}
          />
          <TextField
            label="City"
            required
            placeholder="Portland, OR"
            error={errors.city?.message}
            {...register("city")}
          />
          <SelectField
            label="Property type"
            required
            defaultValue=""
            error={errors.property_type?.message}
            {...register("property_type")}
          >
            <option value="" disabled>
              Select a type…
            </option>
            {PROPERTY_TYPES.map((o) => (
              <option key={o} value={o}>
                {humanize(o)}
              </option>
            ))}
          </SelectField>
          <TextField
            label="Units"
            type="number"
            inputMode="numeric"
            placeholder="12"
            {...register("units")}
          />
          <TextField
            label="Occupied units"
            type="number"
            inputMode="numeric"
            placeholder="11"
            {...register("occupied_units")}
          />
          <TextField
            label="Year built"
            type="number"
            inputMode="numeric"
            placeholder="2019"
            {...register("year_built")}
          />
          <TextField
            label="Manager"
            placeholder="Dana K."
            {...register("manager")}
          />
        </CardContent>
      </Card>

      {/* Financials & strategy */}
      <Card>
        <CardHeader className="block border-b-0 pb-0">
          <CardTitle>Financials &amp; strategy</CardTitle>
          <CardDescription>
            How you run the property and the acquisition basics.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <SelectField
            label="Strategy"
            required
            defaultValue=""
            error={errors.strategy?.message}
            {...register("strategy")}
          >
            <option value="" disabled>
              Select a strategy…
            </option>
            {STRATEGIES.map((o) => (
              <option key={o} value={o}>
                {humanize(o)}
              </option>
            ))}
          </SelectField>
          <TextField
            label="Monthly rent"
            type="number"
            inputMode="decimal"
            placeholder="0.00"
            hint="Gross scheduled rent, in dollars."
            {...register("monthly_rent")}
          />
          <TextField
            label="Purchase price"
            type="number"
            inputMode="decimal"
            placeholder="0.00"
            hint="In dollars."
            {...register("purchase_price")}
          />
          <TextField
            label="Acquired on"
            type="date"
            {...register("acquired_on")}
          />
        </CardContent>
      </Card>

      {/* Holding entity */}
      <Card>
        <CardHeader className="block border-b-0 pb-0">
          <CardTitle>Holding entity</CardTitle>
          <CardDescription>
            Optionally vest this property in one of your LLCs.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <SelectField
            label="Holding LLC"
            defaultValue=""
            disabled={llcs.isLoading}
            className="sm:max-w-sm"
            {...register("llc_id")}
          >
            <option value="">No holding entity</option>
            {(llcs.data ?? []).map((g) => (
              <option key={g.id} value={g.id}>
                {g.name}
                {g.state ? ` · ${g.state}` : ""}
              </option>
            ))}
          </SelectField>
        </CardContent>
      </Card>

      {/* Financing */}
      <Card>
        <CardHeader>
          <div className="min-w-0">
            <CardTitle>Financing</CardTitle>
            <CardDescription className="mt-0.5">
              Optional — add any loans secured against the property.
            </CardDescription>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => append(emptyMortgage())}
          >
            <Plus className="h-4 w-4" />
            Add loan
          </Button>
        </CardHeader>
        <CardContent className="space-y-5">
          {fields.length === 0 ? (
            <p className="text-sm text-ink-3">
              No loans added. Financing is optional — add a loan or onboard
              without one.
            </p>
          ) : (
            fields.map((field, idx) => (
              <div
                key={field.id}
                className="space-y-4 rounded-xl border border-line bg-surface-2/40 p-4"
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-semibold text-ink-2">
                    Loan {idx + 1}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => remove(idx)}
                    aria-label={`Remove loan ${idx + 1}`}
                  >
                    <Trash2 className="h-4 w-4" />
                    Remove
                  </Button>
                </div>
                <div className="grid gap-4 sm:grid-cols-2">
                  <TextField
                    label="Lender name"
                    placeholder="First National"
                    {...register(`mortgages.${idx}.lender_name`)}
                  />
                  <SelectField
                    label="Kind"
                    {...register(`mortgages.${idx}.kind`)}
                  >
                    {MORTGAGE_KINDS.map((o) => (
                      <option key={o} value={o}>
                        {humanize(o)}
                      </option>
                    ))}
                  </SelectField>
                  <TextField
                    label="Original amount"
                    type="number"
                    inputMode="decimal"
                    placeholder="0.00"
                    {...register(`mortgages.${idx}.original_amount`)}
                  />
                  <TextField
                    label="Current balance"
                    type="number"
                    inputMode="decimal"
                    placeholder="0.00"
                    {...register(`mortgages.${idx}.current_balance`)}
                  />
                  <TextField
                    label="Interest rate (%)"
                    type="number"
                    inputMode="decimal"
                    placeholder="6.5"
                    {...register(`mortgages.${idx}.interest_rate`)}
                  />
                  <TextField
                    label="Monthly payment"
                    type="number"
                    inputMode="decimal"
                    placeholder="0.00"
                    {...register(`mortgages.${idx}.monthly_payment`)}
                  />
                  <TextField
                    label="Escrow / mo"
                    type="number"
                    inputMode="decimal"
                    placeholder="0.00"
                    {...register(`mortgages.${idx}.escrow`)}
                  />
                  <TextField
                    label="Term (months)"
                    type="number"
                    inputMode="numeric"
                    placeholder="360"
                    {...register(`mortgages.${idx}.term_months`)}
                  />
                  <TextField
                    label="Loan number"
                    {...register(`mortgages.${idx}.loan_number`)}
                  />
                  <TextField
                    label="Start date"
                    type="date"
                    {...register(`mortgages.${idx}.start_date`)}
                  />
                  <TextField
                    label="Maturity date"
                    type="date"
                    {...register(`mortgages.${idx}.maturity_date`)}
                  />
                </div>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {/* Submit */}
      <Card>
        <CardContent className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <label className="flex items-start gap-3">
            <Switch
              checked={enrich}
              onCheckedChange={(v) =>
                setValue("enrich", v, { shouldDirty: true })
              }
              aria-label="Run automated enrichment"
            />
            <span className="text-sm">
              <span className="font-semibold text-ink">
                Run automated enrichment
              </span>
              <span className="block text-ink-3">
                Fetch parcel records, taxes, and valuations after onboarding.
              </span>
            </span>
          </label>
          <div className="flex items-center gap-2">
            <Button asChild type="button" variant="outline">
              <Link href="/console/properties">Cancel</Link>
            </Button>
            <Button type="submit" disabled={submitting}>
              <Building2 className="h-4 w-4" />
              {submitting ? "Onboarding…" : "Onboard property"}
            </Button>
          </div>
        </CardContent>
      </Card>
    </form>
  );
}
