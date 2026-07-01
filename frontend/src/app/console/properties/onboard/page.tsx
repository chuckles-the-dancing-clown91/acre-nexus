"use client";

// Multi-step property onboarding wizard. Collects property details, optional
// financing (mortgages), and a review/confirm step before calling
// `api.onboardProperty`. Gated behind the "property:write" permission.

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import type { OnboardInput, OnboardMortgageInput } from "@/lib/types";
import { ASSIGNABLE_RELATIONSHIPS } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { useMembers } from "@/lib/queries";
import { Button, Card } from "@/components/ui";

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

// ---- Form state ------------------------------------------------------------
interface PropertyForm {
  name: string;
  address: string;
  city: string;
  property_type: string;
  strategy: string;
  units: string;
  occupied_units: string;
  year_built: string;
  monthly_rent: string;
  purchase_price: string;
  acquired_on: string;
}

interface MortgageForm {
  lender_name: string;
  kind: string;
  original_amount: string;
  current_balance: string;
  interest_rate: string;
  monthly_payment: string;
  escrow: string;
  term_months: string;
  loan_number: string;
  start_date: string;
  maturity_date: string;
}

const EMPTY_PROPERTY: PropertyForm = {
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
};

function emptyMortgage(): MortgageForm {
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

const FIELD_CLASS =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink";

const STEPS = ["Property", "Financing", "Team", "Review"] as const;

/** A row in the onboarding Team step. */
interface TeamRow {
  user_id: string;
  relationship: string;
  is_primary: boolean;
}

// ---- Conversion helpers ----------------------------------------------------
/** Dollars string → integer cents, or undefined if blank/NaN. */
function dollarsToCents(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  if (Number.isNaN(n)) return undefined;
  return Math.round(n * 100);
}

/** Percent string (e.g. "6.5") → integer basis points, or undefined. */
function percentToBps(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  if (Number.isNaN(n)) return undefined;
  return Math.round(n * 100);
}

/** Integer string → number, or undefined if blank/NaN. */
function toInt(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  if (Number.isNaN(n)) return undefined;
  return Math.round(n);
}

/** Trimmed string, or undefined if blank. */
function toStr(v: string): string | undefined {
  const t = v.trim();
  return t === "" ? undefined : t;
}

/** Turn a snake/lower key into a human label, e.g. `single_family` → `Single family`. */
function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** True when a mortgage row has no meaningful data entered. */
function isMortgageEmpty(m: MortgageForm): boolean {
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

/** Map a populated mortgage form row to the API input, dropping empty fields. */
function toMortgageInput(m: MortgageForm): OnboardMortgageInput {
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

// ---- Small field components ------------------------------------------------
function TextField({
  label,
  value,
  onChange,
  required,
  type = "text",
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  required?: boolean;
  type?: string;
  placeholder?: string;
}) {
  return (
    <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
      <span>
        {label}
        {required && <span className="ml-0.5 text-bad">*</span>}
      </span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className={FIELD_CLASS}
      />
    </label>
  );
}

function SelectField({
  label,
  value,
  onChange,
  options,
  required,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  options: readonly string[];
  required?: boolean;
  placeholder?: string;
}) {
  return (
    <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
      <span>
        {label}
        {required && <span className="ml-0.5 text-bad">*</span>}
      </span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={FIELD_CLASS}
      >
        {placeholder !== undefined && <option value="">{placeholder}</option>}
        {options.map((o) => (
          <option key={o} value={o}>
            {humanize(o)}
          </option>
        ))}
      </select>
    </label>
  );
}

function SummaryRow({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-center justify-between border-b border-line pb-2.5 last:border-0">
      <dt className="text-ink-3">{k}</dt>
      <dd className="font-semibold">{v}</dd>
    </div>
  );
}

// ---- Page ------------------------------------------------------------------
export default function OnboardPropertyPage() {
  const { can } = useAuth();
  const router = useRouter();

  const [step, setStep] = useState<1 | 2 | 3 | 4>(1);
  const [property, setProperty] = useState<PropertyForm>(EMPTY_PROPERTY);
  const [mortgages, setMortgages] = useState<MortgageForm[]>([]);
  const [team, setTeam] = useState<TeamRow[]>([]);
  const [enrich, setEnrich] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { data: members } = useMembers();

  if (!can("property:write")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to onboard properties. Ask a platform admin
          to grant the <span className="font-mono">property:write</span>{" "}
          permission.
        </p>
      </Card>
    );
  }

  const setP = (patch: Partial<PropertyForm>) =>
    setProperty((prev) => ({ ...prev, ...patch }));

  const setMortgage = (idx: number, patch: Partial<MortgageForm>) =>
    setMortgages((prev) =>
      prev.map((m, i) => (i === idx ? { ...m, ...patch } : m))
    );

  const addMortgage = () => setMortgages((prev) => [...prev, emptyMortgage()]);

  const removeMortgage = (idx: number) =>
    setMortgages((prev) => prev.filter((_, i) => i !== idx));

  const addPerson = () =>
    setTeam((prev) => [
      ...prev,
      {
        user_id: "",
        relationship: ASSIGNABLE_RELATIONSHIPS[0].key,
        is_primary: false,
      },
    ]);

  const setPerson = (idx: number, patch: Partial<TeamRow>) =>
    setTeam((prev) => prev.map((t, i) => (i === idx ? { ...t, ...patch } : t)));

  const removePerson = (idx: number) =>
    setTeam((prev) => prev.filter((_, i) => i !== idx));

  const populatedTeam = team.filter((t) => t.user_id !== "");

  const step1Valid =
    property.name.trim() !== "" &&
    property.address.trim() !== "" &&
    property.city.trim() !== "" &&
    property.property_type !== "" &&
    property.strategy !== "";

  const buildInput = (): OnboardInput => {
    const input: OnboardInput = {
      name: property.name.trim(),
      address: property.address.trim(),
      city: property.city.trim(),
      property_type: property.property_type,
      strategy: property.strategy,
      mortgages: mortgages
        .filter((m) => !isMortgageEmpty(m))
        .map(toMortgageInput),
      enrich,
    };
    const units = toInt(property.units);
    if (units !== undefined) input.units = units;
    const occupied = toInt(property.occupied_units);
    if (occupied !== undefined) input.occupied_units = occupied;
    const rent = dollarsToCents(property.monthly_rent);
    if (rent !== undefined) input.monthly_rent_cents = rent;
    const year = toInt(property.year_built);
    if (year !== undefined) input.year_built = year;
    const price = dollarsToCents(property.purchase_price);
    if (price !== undefined) input.purchase_price_cents = price;
    const acquired = toStr(property.acquired_on);
    if (acquired !== undefined) input.acquired_on = acquired;
    if (populatedTeam.length > 0) {
      input.assignments = populatedTeam.map((t) => ({
        user_id: t.user_id,
        relationship: t.relationship,
        is_primary: t.is_primary,
      }));
    }
    return input;
  };

  const submit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const resp = await api.onboardProperty(buildInput());
      router.push(`/console/properties/${resp.property_id}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to onboard property.");
      setSubmitting(false);
    }
  };

  const populatedMortgages = mortgages.filter((m) => !isMortgageEmpty(m));

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Onboard a property
        </h1>
        <p className="text-ink-3">
          Add property details, financing, and kick off enrichment in three
          steps.
        </p>
      </div>

      {/* Step indicator */}
      <div className="flex flex-wrap items-center gap-3">
        {STEPS.map((label, i) => {
          const n = (i + 1) as 1 | 2 | 3 | 4;
          const active = n === step;
          const done = n < step;
          return (
            <div key={label} className="flex items-center gap-2">
              <span
                className={
                  "flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold " +
                  (active
                    ? "bg-accent text-on-accent"
                    : done
                      ? "bg-good-soft text-good"
                      : "bg-surface-2 text-ink-3")
                }
              >
                {n}
              </span>
              <span
                className={
                  "text-sm font-semibold " +
                  (active ? "text-ink" : "text-ink-3")
                }
              >
                {label}
              </span>
              {i < STEPS.length - 1 && (
                <span className="ml-1 hidden h-px w-8 bg-line sm:block" />
              )}
            </div>
          );
        })}
      </div>

      {/* Step 1: Property */}
      {step === 1 && (
        <Card className="space-y-5 p-5">
          <h2 className="font-display text-lg font-bold">Property details</h2>
          <div className="grid gap-4 sm:grid-cols-2">
            <TextField
              label="Name"
              value={property.name}
              onChange={(v) => setP({ name: v })}
              required
            />
            <TextField
              label="Address"
              value={property.address}
              onChange={(v) => setP({ address: v })}
              required
            />
            <TextField
              label="City"
              value={property.city}
              onChange={(v) => setP({ city: v })}
              required
            />
            <SelectField
              label="Property type"
              value={property.property_type}
              onChange={(v) => setP({ property_type: v })}
              options={PROPERTY_TYPES}
              placeholder="Select a type…"
              required
            />
            <SelectField
              label="Strategy"
              value={property.strategy}
              onChange={(v) => setP({ strategy: v })}
              options={STRATEGIES}
              placeholder="Select a strategy…"
              required
            />
            <TextField
              label="Units"
              type="number"
              value={property.units}
              onChange={(v) => setP({ units: v })}
            />
            <TextField
              label="Occupied units"
              type="number"
              value={property.occupied_units}
              onChange={(v) => setP({ occupied_units: v })}
            />
            <TextField
              label="Year built"
              type="number"
              value={property.year_built}
              onChange={(v) => setP({ year_built: v })}
            />
            <TextField
              label="Monthly rent ($)"
              type="number"
              value={property.monthly_rent}
              onChange={(v) => setP({ monthly_rent: v })}
            />
            <TextField
              label="Purchase price ($)"
              type="number"
              value={property.purchase_price}
              onChange={(v) => setP({ purchase_price: v })}
            />
            <TextField
              label="Acquired on"
              type="date"
              value={property.acquired_on}
              onChange={(v) => setP({ acquired_on: v })}
            />
          </div>
          <div className="flex justify-end">
            <Button onClick={() => setStep(2)} disabled={!step1Valid}>
              Next
            </Button>
          </div>
        </Card>
      )}

      {/* Step 2: Financing */}
      {step === 2 && (
        <Card className="space-y-5 p-5">
          <div className="flex items-center justify-between">
            <h2 className="font-display text-lg font-bold">Financing</h2>
            <Button variant="outline" onClick={addMortgage}>
              Add loan
            </Button>
          </div>

          {mortgages.length === 0 ? (
            <p className="text-sm text-ink-3">
              No loans added. Financing is optional — add a loan or continue.
            </p>
          ) : (
            <div className="space-y-5">
              {mortgages.map((m, idx) => (
                <div
                  key={idx}
                  className="space-y-4 rounded-xl border border-line p-4"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-bold text-ink-2">
                      Loan {idx + 1}
                    </span>
                    <Button variant="ghost" onClick={() => removeMortgage(idx)}>
                      Remove
                    </Button>
                  </div>
                  <div className="grid gap-4 sm:grid-cols-2">
                    <TextField
                      label="Lender name"
                      value={m.lender_name}
                      onChange={(v) => setMortgage(idx, { lender_name: v })}
                    />
                    <SelectField
                      label="Kind"
                      value={m.kind}
                      onChange={(v) => setMortgage(idx, { kind: v })}
                      options={MORTGAGE_KINDS}
                    />
                    <TextField
                      label="Original amount ($)"
                      type="number"
                      value={m.original_amount}
                      onChange={(v) => setMortgage(idx, { original_amount: v })}
                    />
                    <TextField
                      label="Current balance ($)"
                      type="number"
                      value={m.current_balance}
                      onChange={(v) => setMortgage(idx, { current_balance: v })}
                    />
                    <TextField
                      label="Interest rate (%)"
                      type="number"
                      value={m.interest_rate}
                      onChange={(v) => setMortgage(idx, { interest_rate: v })}
                    />
                    <TextField
                      label="Monthly payment ($)"
                      type="number"
                      value={m.monthly_payment}
                      onChange={(v) => setMortgage(idx, { monthly_payment: v })}
                    />
                    <TextField
                      label="Escrow / mo ($)"
                      type="number"
                      value={m.escrow}
                      onChange={(v) => setMortgage(idx, { escrow: v })}
                    />
                    <TextField
                      label="Term (months)"
                      type="number"
                      value={m.term_months}
                      onChange={(v) => setMortgage(idx, { term_months: v })}
                    />
                    <TextField
                      label="Loan number"
                      value={m.loan_number}
                      onChange={(v) => setMortgage(idx, { loan_number: v })}
                    />
                    <TextField
                      label="Start date"
                      type="date"
                      value={m.start_date}
                      onChange={(v) => setMortgage(idx, { start_date: v })}
                    />
                    <TextField
                      label="Maturity date"
                      type="date"
                      value={m.maturity_date}
                      onChange={(v) => setMortgage(idx, { maturity_date: v })}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}

          <div className="flex justify-between">
            <Button variant="outline" onClick={() => setStep(1)}>
              Back
            </Button>
            <Button onClick={() => setStep(3)}>Next</Button>
          </div>
        </Card>
      )}

      {/* Step 3: Team */}
      {step === 3 && (
        <Card className="space-y-5 p-5">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="font-display text-lg font-bold">Team</h2>
              <p className="text-sm text-ink-3">
                Assign staff to this property. Each assignment also grants that
                person access to it. Optional — you can add more later.
              </p>
            </div>
            <Button variant="outline" onClick={addPerson}>
              Add person
            </Button>
          </div>

          {team.length === 0 ? (
            <p className="text-sm text-ink-3">
              No one assigned. Add a property manager or landlord, or continue.
            </p>
          ) : (
            <div className="space-y-4">
              {team.map((t, idx) => (
                <div
                  key={idx}
                  className="grid items-end gap-4 rounded-xl border border-line p-4 sm:grid-cols-[1.4fr_1fr_auto]"
                >
                  <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                    <span>Person</span>
                    <select
                      value={t.user_id}
                      onChange={(e) =>
                        setPerson(idx, { user_id: e.target.value })
                      }
                      className={FIELD_CLASS}
                    >
                      <option value="">— Select member —</option>
                      {(members ?? []).map((m) => (
                        <option key={m.user_id} value={m.user_id}>
                          {m.name} · {m.email}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
                    <span>Relationship</span>
                    <select
                      value={t.relationship}
                      onChange={(e) =>
                        setPerson(idx, { relationship: e.target.value })
                      }
                      className={FIELD_CLASS}
                    >
                      {ASSIGNABLE_RELATIONSHIPS.map((r) => (
                        <option key={r.key} value={r.key}>
                          {r.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="flex items-center gap-3 pb-1">
                    <label className="flex items-center gap-1.5 text-xs font-semibold text-ink-2">
                      <input
                        type="checkbox"
                        checked={t.is_primary}
                        onChange={(e) =>
                          setPerson(idx, { is_primary: e.target.checked })
                        }
                      />
                      Primary
                    </label>
                    <Button variant="ghost" onClick={() => removePerson(idx)}>
                      Remove
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}

          <div className="flex justify-between">
            <Button variant="outline" onClick={() => setStep(2)}>
              Back
            </Button>
            <Button onClick={() => setStep(4)}>Next</Button>
          </div>
        </Card>
      )}

      {/* Step 4: Review & submit */}
      {step === 4 && (
        <Card className="space-y-5 p-5">
          <h2 className="font-display text-lg font-bold">
            Review &amp; submit
          </h2>

          <dl className="space-y-3 text-sm">
            <SummaryRow k="Name" v={property.name} />
            <SummaryRow k="Address" v={property.address} />
            <SummaryRow k="City" v={property.city} />
            <SummaryRow
              k="Property type"
              v={humanize(property.property_type)}
            />
            <SummaryRow k="Strategy" v={humanize(property.strategy)} />
            {property.units.trim() !== "" && (
              <SummaryRow k="Units" v={property.units} />
            )}
            {property.occupied_units.trim() !== "" && (
              <SummaryRow k="Occupied units" v={property.occupied_units} />
            )}
            {property.year_built.trim() !== "" && (
              <SummaryRow k="Year built" v={property.year_built} />
            )}
            {property.monthly_rent.trim() !== "" && (
              <SummaryRow k="Monthly rent" v={`$${property.monthly_rent}`} />
            )}
            {property.purchase_price.trim() !== "" && (
              <SummaryRow
                k="Purchase price"
                v={`$${property.purchase_price}`}
              />
            )}
            {property.acquired_on.trim() !== "" && (
              <SummaryRow k="Acquired on" v={property.acquired_on} />
            )}
          </dl>

          <div>
            <h3 className="mb-2 text-sm font-bold text-ink-2">
              Financing ({populatedMortgages.length})
            </h3>
            {populatedMortgages.length === 0 ? (
              <p className="text-sm text-ink-3">No loans added.</p>
            ) : (
              <div className="space-y-3">
                {populatedMortgages.map((m, idx) => (
                  <div
                    key={idx}
                    className="rounded-xl border border-line p-3 text-sm"
                  >
                    <div className="mb-1 font-semibold">
                      {m.lender_name.trim() || "Loan"} · {humanize(m.kind)}
                    </div>
                    <div className="text-ink-3">
                      {m.original_amount.trim() !== "" &&
                        `Orig $${m.original_amount} · `}
                      {m.current_balance.trim() !== "" &&
                        `Balance $${m.current_balance} · `}
                      {m.interest_rate.trim() !== "" &&
                        `${m.interest_rate}% · `}
                      {m.monthly_payment.trim() !== "" &&
                        `$${m.monthly_payment}/mo`}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div>
            <h3 className="mb-2 text-sm font-bold text-ink-2">
              Team ({populatedTeam.length})
            </h3>
            {populatedTeam.length === 0 ? (
              <p className="text-sm text-ink-3">No one assigned.</p>
            ) : (
              <div className="space-y-2">
                {populatedTeam.map((t, idx) => {
                  const m = (members ?? []).find(
                    (x) => x.user_id === t.user_id
                  );
                  const rel = ASSIGNABLE_RELATIONSHIPS.find(
                    (r) => r.key === t.relationship
                  );
                  return (
                    <div
                      key={idx}
                      className="rounded-xl border border-line p-3 text-sm"
                    >
                      <span className="font-semibold">
                        {m?.name ?? "Member"}
                      </span>{" "}
                      <span className="text-ink-3">
                        · {rel?.label ?? t.relationship}
                        {t.is_primary ? " · Primary" : ""}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <label className="flex items-center gap-2 text-sm font-semibold text-ink-2">
            <input
              type="checkbox"
              checked={enrich}
              onChange={(e) => setEnrich(e.target.checked)}
              className="h-4 w-4 rounded border-line"
            />
            Run automated enrichment after onboarding
          </label>

          {error && <p className="text-bad">{error}</p>}

          <div className="flex justify-between">
            <Button
              variant="outline"
              onClick={() => setStep(3)}
              disabled={submitting}
            >
              Back
            </Button>
            <Button onClick={submit} disabled={submitting}>
              {submitting ? "Onboarding…" : "Onboard property"}
            </Button>
          </div>
        </Card>
      )}
    </div>
  );
}
