"use client";

// The card beside the photos: what it's worth, what it rents for, the facts
// at a glance, and the month in numbers.

import { useQuery } from "@tanstack/react-query";
import { MapPin } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import {
  compactUsd,
  grossYieldPct,
  monthlyPicture,
  perSqft,
} from "@/lib/propertyMath";
import { records } from "@/lib/propertyRecords";
import type { PropertyProfile } from "@/lib/types";
import { Badge, statusTone } from "@/components/ui/badge";
import { Panel } from "@/components/ui/panel";
import { Skeleton } from "@/components/ui/misc";
import { cn } from "@/lib/utils";

export function Summary({
  property: p,
  propertyId,
}: {
  property: PropertyProfile | undefined;
  propertyId: string;
}) {
  const { can } = useAuth();
  const intel = useQuery({
    queryKey: ["intel", propertyId],
    queryFn: () => api.propertyIntel(propertyId),
    enabled: !!p,
  });
  const policies = useQuery({
    queryKey: ["policies", propertyId],
    queryFn: () => records.policies(propertyId),
    enabled: !!p && can("property:read"),
    retry: false,
  });

  if (!p) return <Skeleton className="h-96 rounded-2xl" />;

  const d = intel.data?.detail;
  const value = intel.data?.valuations[0];
  const tax = intel.data?.taxes[0];
  const insurance = (policies.data ?? [])
    .filter((x) => x.status === "active")
    .reduce((s, x) => s + (x.premium_cents ?? 0), 0);
  const month = monthlyPicture({
    rentMonthly: p.monthly_rent_cents,
    loanMonthly: p.financed ? p.debt_service_cents : 0,
    taxYearly: tax?.tax_amount_cents,
    insuranceYearly: insurance,
  });
  const yieldPct = grossYieldPct(
    p.monthly_rent_cents,
    value?.estimated_value_cents
  );
  const psf = perSqft(value?.estimated_value_cents, d?.sqft);
  const facts = [
    d?.beds != null && { n: String(d.beds), l: "bd" },
    d?.baths != null && { n: String(d.baths), l: "ba" },
    d?.sqft != null && { n: d.sqft.toLocaleString(), l: "sqft" },
  ].filter(Boolean) as { n: string; l: string }[];

  return (
    <Panel className="flex h-full flex-col p-5">
      <div className="flex flex-wrap gap-1.5">
        <Badge tone={statusTone(p.status)}>{p.status}</Badge>
        {p.property_type && <Badge>{p.property_type.replace(/_/g, " ")}</Badge>}
        {p.year_built > 0 && <Badge>Built {p.year_built}</Badge>}
      </div>
      <h1 className="mt-3 text-[24px] leading-tight font-semibold text-fg">
        {p.name}
      </h1>
      <div className="mt-1 flex items-start gap-1.5 text-[13px] text-fg-3">
        <MapPin className="mt-0.5 size-4 shrink-0" />
        {[p.address, p.city, [p.state, p.postal_code].filter(Boolean).join(" ")]
          .filter(Boolean)
          .join(", ")}
      </div>

      <div className="mt-5 grid grid-cols-2 gap-4">
        <div>
          <div className="eyebrow">Estimated value</div>
          <div className="figure mt-1 text-[28px] leading-none font-semibold text-fg">
            {value?.estimated_value_cents
              ? compactUsd(value.estimated_value_cents)
              : "—"}
          </div>
          {value?.value_low_cents && value.value_high_cents && (
            <div className="mt-1 text-[11px] text-fg-3">
              {compactUsd(value.value_low_cents)} to{" "}
              {compactUsd(value.value_high_cents)}
            </div>
          )}
        </div>
        <div>
          <div className="eyebrow">Rent roll</div>
          <div className="figure mt-1 text-[28px] leading-none font-semibold text-fg">
            {usd(p.monthly_rent_cents)}
          </div>
          <div className="mt-1 text-[11px] text-fg-3">
            {value?.estimated_rent_cents
              ? `market about ${usd(value.estimated_rent_cents)}`
              : "per month"}
          </div>
        </div>
      </div>

      {facts.length > 0 && (
        <div className="mt-4 flex divide-x divide-line rounded-xl border border-line">
          {facts.map((f) => (
            <div key={f.l} className="flex-1 px-3 py-2 text-center">
              <span className="figure text-[16px] font-semibold text-fg">
                {f.n}
              </span>{" "}
              <span className="text-xs text-fg-3">{f.l}</span>
            </div>
          ))}
        </div>
      )}

      <dl className="mt-4 grid grid-cols-3 gap-2 text-center">
        <Mini
          label="Gross yield"
          value={yieldPct != null ? `${yieldPct}%` : "—"}
        />
        <Mini label="Per sq ft" value={psf != null ? `$${psf}` : "—"} />
        <Mini label="Occupied" value={`${p.occupied_units}/${p.units}`} />
      </dl>

      <div className="mt-5 border-t border-line pt-4">
        <div className="eyebrow mb-2">The month</div>
        <ul className="space-y-1 text-[13px]">
          <Row label="Rent" cents={month.rent} />
          {month.loan > 0 && <Row label="Loan payment" cents={-month.loan} />}
          {month.taxes > 0 && <Row label="Property tax" cents={-month.taxes} />}
          {month.insurance > 0 && (
            <Row label="Insurance" cents={-month.insurance} />
          )}
          <li className="flex justify-between border-t border-line pt-1.5 font-medium">
            <span className="text-fg">Left over</span>
            <span
              className={cn("figure", month.net < 0 ? "text-bad" : "text-good")}
            >
              {usd(month.net)}
            </span>
          </li>
        </ul>
        <p className="mt-2 text-[11px] text-fg-4">
          Before upkeep and management. Taxes and insurance are the latest on
          file, spread over twelve months.
        </p>
      </div>
    </Panel>
  );
}

function Mini({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl bg-fill/50 px-2 py-2">
      <div className="text-[11px] text-fg-3">{label}</div>
      <div className="figure text-[15px] font-semibold text-fg">{value}</div>
    </div>
  );
}

function Row({ label, cents }: { label: string; cents: number }) {
  return (
    <li className="flex justify-between text-fg-2">
      <span>{label}</span>
      <span className="figure">
        {cents < 0 ? `−${usd(-cents)}` : usd(cents)}
      </span>
    </li>
  );
}
