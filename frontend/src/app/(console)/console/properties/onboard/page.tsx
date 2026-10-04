"use client";

// Add a property. What it is comes first, because that decides the rest: a
// house is one unit, a building has a list of them, a campground has sites
// drawn on its map afterwards.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  ArrowRight,
  Building,
  Building2,
  Home,
  Landmark,
  Mountain,
  Tent,
  Trees,
  Warehouse,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { KINDS, type Kind } from "@/lib/propertyKind";
import { dollarsToCents } from "@/lib/showings";
import { Button } from "@/components/ui/button";
import { PageHeader } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { Steps } from "@/components/ui/steps";
import { F, input, why } from "@/components/property/bits";
import { cn } from "@/lib/utils";

const ICON: Record<string, React.ComponentType<{ className?: string }>> = {
  single_family: Home,
  townhome: Home,
  condo: Building,
  manufactured: Home,
  multi_family: Building2,
  commercial: Warehouse,
  campground: Tent,
  rv_park: Mountain,
  land: Trees,
};

const BLURB: Record<string, string> = {
  single_family: "One house, one tenant.",
  townhome: "One townhome.",
  condo: "One condo.",
  manufactured: "One manufactured home.",
  multi_family: "Apartments: many units, each with its own tenant.",
  commercial: "Suites for businesses.",
  campground: "Sites by the night.",
  rv_park: "Full-hookup sites by the night.",
  land: "Acreage to hold or sell.",
};

const STEPS = ["Type", "Address", "Layout", "Review"];

export default function AddPropertyPage() {
  const { can } = useAuth();
  const router = useRouter();
  const llcs = useQuery({
    queryKey: ["legal-entities"],
    queryFn: api.legalEntities,
  });
  const [step, setStep] = useState(0);
  const [kind, setKind] = useState<Kind | null>(null);
  const [f, setF] = useState({
    llc_id: "",
    strategy: "rental",
    name: "",
    address: "",
    city: "",
    state: "",
    postal_code: "",
    year_built: "",
    manager: "",
    // a house's layout
    beds: "",
    baths: "",
    sqft: "",
    rent: "",
    // a building's units
    unit_prefix: "",
    unit_count: "",
    unit_beds: "",
    unit_rent: "",
    unit_names: "",
    enrich: true,
  });
  const [busy, setBusy] = useState(false);
  const set = (patch: Partial<typeof f>) => setF((p) => ({ ...p, ...patch }));

  // A building's unit numbers: typed one a line, or counted up from a prefix.
  const unitNames = useMemo(() => {
    const typed = f.unit_names
      .split(/\n|,/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (typed.length) return typed;
    const n = Math.min(Number(f.unit_count) || 0, 200);
    return Array.from({ length: n }, (_, i) => `${f.unit_prefix}${i + 1}`);
  }, [f.unit_names, f.unit_count, f.unit_prefix]);

  if (!can("property:write"))
    return (
      <Panel className="mx-auto mt-10 max-w-lg p-6 text-center text-[14px] text-fg-2">
        Adding a property needs permission to edit properties.
      </Panel>
    );

  const rentCents = f.rent ? dollarsToCents(f.rent) : null;
  const unitRentCents = f.unit_rent ? dollarsToCents(f.unit_rent) : null;
  const needsAddress = !f.name.trim() || !f.address.trim() || !f.city.trim();
  const layoutBad =
    !!kind &&
    ((kind.mode === "single" && ((f.rent && rentCents == null) || false)) ||
      (kind.mode === "multi" &&
        (unitNames.length === 0 || (f.unit_rent && unitRentCents == null))));

  function next() {
    if (step === 0 && !kind) return void toast.error("Pick what you're adding");
    if (step === 1 && needsAddress)
      return void toast.error("Give it a name, an address and a city");
    if (step === 2 && layoutBad)
      return void toast.error(
        kind?.mode === "multi"
          ? "List the units, and enter rent as dollars"
          : "Enter rent as dollars"
      );
    setStep(step + 1);
  }

  async function create() {
    if (!kind) return;
    setBusy(true);
    try {
      const units =
        kind.mode === "multi"
          ? unitNames.map((n) => ({
              unit_number: n,
              ...(f.unit_beds ? { beds: Number(f.unit_beds) } : {}),
              ...(unitRentCents != null
                ? { market_rent_cents: unitRentCents }
                : {}),
            }))
          : kind.mode === "single" &&
              (f.beds || f.baths || f.sqft || rentCents != null)
            ? [
                {
                  unit_number: "Home",
                  ...(f.beds ? { beds: Number(f.beds) } : {}),
                  ...(f.baths ? { baths: Number(f.baths) } : {}),
                  ...(f.sqft ? { sqft: Number(f.sqft) } : {}),
                  ...(rentCents != null
                    ? { market_rent_cents: rentCents }
                    : {}),
                },
              ]
            : [];
      const r = await api.onboardProperty({
        name: f.name.trim(),
        address: f.address.trim(),
        city: f.city.trim(),
        ...(f.state ? { state: f.state } : {}),
        ...(f.postal_code ? { postal_code: f.postal_code } : {}),
        ...(f.llc_id ? { llc_id: f.llc_id } : {}),
        units:
          kind.mode === "multi"
            ? unitNames.length
            : kind.mode === "single"
              ? 1
              : 0,
        occupied_units: 0,
        monthly_rent_cents: 0,
        ...(f.year_built ? { year_built: Number(f.year_built) } : {}),
        ...(f.manager ? { manager: f.manager } : {}),
        property_type: kind.key,
        strategy: f.strategy,
        unit_list: units,
        mortgages: [],
        enrich: f.enrich,
      });
      toast.success("Property added");
      router.push(`/console/properties/${r.property_id}`);
    } catch (e) {
      toast.error(why(e));
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <Link
        href="/console/properties"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Properties
      </Link>
      <PageHeader
        eyebrow="Properties"
        title="Add a property"
        description="What it is decides how it's set up."
      />
      <Steps steps={STEPS} current={step} />

      <Panel className="space-y-5 p-6">
        {step === 0 && (
          <>
            <div className="grid gap-3 sm:grid-cols-2">
              {KINDS.map((k) => {
                const Icon = ICON[k.key] ?? Landmark;
                const on = kind?.key === k.key;
                return (
                  <button
                    key={k.key}
                    type="button"
                    onClick={() => setKind(k)}
                    aria-pressed={on}
                    className={cn(
                      "flex items-start gap-3 rounded-xl border p-4 text-left transition",
                      on
                        ? "border-accent bg-accent/5"
                        : "border-line hover:border-fg-4"
                    )}
                  >
                    <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-fill text-fg-2">
                      <Icon className="size-4.5" />
                    </span>
                    <span>
                      <span className="block text-[14px] font-semibold text-fg">
                        {k.label}
                      </span>
                      <span className="block text-[12px] text-fg-3">
                        {BLURB[k.key]}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <F label="Which company owns it">
                <select
                  className={input}
                  value={f.llc_id}
                  onChange={(e) => set({ llc_id: e.target.value })}
                >
                  <option value="">Not set yet</option>
                  {llcs.data?.map((l) => (
                    <option key={l.id} value={l.id}>
                      {l.name}
                    </option>
                  ))}
                </select>
              </F>
              <F label="What's the plan">
                <select
                  className={input}
                  value={f.strategy}
                  onChange={(e) => set({ strategy: e.target.value })}
                >
                  <option value="rental">Rent it out</option>
                  <option value="flip">Fix and sell</option>
                  <option value="brrrr">Fix, rent and refinance</option>
                  <option value="hold">Hold</option>
                </select>
              </F>
            </div>
          </>
        )}

        {step === 1 && (
          <div className="grid gap-3 sm:grid-cols-2">
            <F label="Name" className="sm:col-span-2">
              <input
                className={input}
                placeholder="Maple Court"
                value={f.name}
                onChange={(e) => set({ name: e.target.value })}
              />
            </F>
            <F label="Street address" className="sm:col-span-2">
              <input
                className={input}
                value={f.address}
                onChange={(e) => set({ address: e.target.value })}
              />
            </F>
            <F label="City">
              <input
                className={input}
                value={f.city}
                onChange={(e) => set({ city: e.target.value })}
              />
            </F>
            <div className="grid grid-cols-2 gap-3">
              <F label="State">
                <input
                  className={input}
                  maxLength={2}
                  value={f.state}
                  onChange={(e) => set({ state: e.target.value.toUpperCase() })}
                />
              </F>
              <F label="ZIP">
                <input
                  className={input}
                  value={f.postal_code}
                  onChange={(e) => set({ postal_code: e.target.value })}
                />
              </F>
            </div>
            <F label="Year built">
              <input
                type="number"
                className={input}
                value={f.year_built}
                onChange={(e) => set({ year_built: e.target.value })}
              />
            </F>
            <F label="Manager">
              <input
                className={input}
                value={f.manager}
                onChange={(e) => set({ manager: e.target.value })}
              />
            </F>
          </div>
        )}

        {step === 2 && kind && (
          <>
            {kind.mode === "single" && (
              <div className="space-y-3">
                <p className="text-[13px] text-fg-3">
                  This is one {kind.noun}, so it&apos;s set up as a single unit.
                  Say what&apos;s in it.
                </p>
                <div className="grid gap-3 sm:grid-cols-4">
                  <F label="Bedrooms">
                    <input
                      type="number"
                      min={0}
                      className={input}
                      value={f.beds}
                      onChange={(e) => set({ beds: e.target.value })}
                    />
                  </F>
                  <F label="Bathrooms">
                    <input
                      type="number"
                      min={0}
                      step="0.5"
                      className={input}
                      value={f.baths}
                      onChange={(e) => set({ baths: e.target.value })}
                    />
                  </F>
                  <F label="Square feet">
                    <input
                      type="number"
                      min={0}
                      className={input}
                      value={f.sqft}
                      onChange={(e) => set({ sqft: e.target.value })}
                    />
                  </F>
                  <F label="Market rent ($)">
                    <input
                      className={input}
                      inputMode="decimal"
                      value={f.rent}
                      onChange={(e) => set({ rent: e.target.value })}
                    />
                  </F>
                </div>
              </div>
            )}
            {kind.mode === "multi" && (
              <div className="space-y-3">
                <p className="text-[13px] text-fg-3">
                  List the {kind.units.toLowerCase()}, one a line, or count them
                  up from a prefix. You can add and change them later.
                </p>
                <div className="grid gap-3 sm:grid-cols-3">
                  <F label="How many">
                    <input
                      type="number"
                      min={1}
                      max={200}
                      className={input}
                      value={f.unit_count}
                      onChange={(e) => set({ unit_count: e.target.value })}
                    />
                  </F>
                  <F label="Number prefix">
                    <input
                      className={input}
                      placeholder="A-"
                      value={f.unit_prefix}
                      onChange={(e) => set({ unit_prefix: e.target.value })}
                    />
                  </F>
                  <F label="Usual bedrooms">
                    <input
                      type="number"
                      min={0}
                      className={input}
                      value={f.unit_beds}
                      onChange={(e) => set({ unit_beds: e.target.value })}
                    />
                  </F>
                  <F label="Usual rent ($)" className="sm:col-span-3">
                    <input
                      className={cn(input, "sm:max-w-48")}
                      inputMode="decimal"
                      value={f.unit_rent}
                      onChange={(e) => set({ unit_rent: e.target.value })}
                    />
                  </F>
                </div>
                <F label={`Or type them out (${unitNames.length} so far)`}>
                  <textarea
                    rows={4}
                    className={input}
                    placeholder={"101\n102\n201"}
                    value={f.unit_names}
                    onChange={(e) => set({ unit_names: e.target.value })}
                  />
                </F>
              </div>
            )}
            {kind.mode === "sites" && (
              <p className="text-[13px] text-fg-2">
                A {kind.noun} has sites, not units. After it&apos;s added you
                draw each site on its site map, set rates, and open booking from
                Campgrounds.
              </p>
            )}
            {kind.mode === "none" && (
              <p className="text-[13px] text-fg-2">
                Land has nothing to lay out yet. You can add acreage and zoning
                on the deal or the property.
              </p>
            )}
          </>
        )}

        {step === 3 && kind && (
          <div className="space-y-3 text-[14px]">
            <dl className="divide-y divide-line rounded-xl border border-line">
              {(
                [
                  ["Type", kind.label],
                  ["Name", f.name],
                  [
                    "Address",
                    [f.address, f.city, f.state, f.postal_code]
                      .filter(Boolean)
                      .join(", "),
                  ],
                  [
                    "Company",
                    llcs.data?.find((l) => l.id === f.llc_id)?.name ??
                      "Not set yet",
                  ],
                  [
                    kind.units,
                    kind.mode === "multi"
                      ? `${unitNames.length}: ${unitNames.slice(0, 6).join(", ")}${unitNames.length > 6 ? "…" : ""}`
                      : kind.mode === "single"
                        ? "1 (the home)"
                        : kind.mode === "sites"
                          ? "Drawn on the site map"
                          : "None",
                  ],
                ] as const
              ).map(([k, v]) => (
                <div key={k} className="flex justify-between gap-4 px-4 py-2.5">
                  <dt className="text-fg-3">{k}</dt>
                  <dd className="text-right font-medium text-fg">{v}</dd>
                </div>
              ))}
            </dl>
            <label className="flex items-center gap-2 text-[13px] text-fg-2">
              <input
                type="checkbox"
                className="size-4 accent-[var(--accent)]"
                checked={f.enrich}
                onChange={(e) => set({ enrich: e.target.checked })}
              />
              Look up the parcel, taxes, schools and photo from public records
            </label>
          </div>
        )}

        <div className="flex items-center justify-between border-t border-line pt-4">
          <Button
            variant="ghost"
            disabled={step === 0}
            onClick={() => setStep(step - 1)}
          >
            <ArrowLeft />
            Back
          </Button>
          {step < STEPS.length - 1 ? (
            <Button onClick={next}>
              Next
              <ArrowRight />
            </Button>
          ) : (
            <Button loading={busy} onClick={() => void create()}>
              Add property
            </Button>
          )}
        </div>
      </Panel>
    </div>
  );
}
