// The numbers beside a property's photos: what it earns against what it
// costs each month, and how that sits against what it's worth.

export interface Monthly {
  rent: number;
  loan: number;
  taxes: number;
  insurance: number;
  /** What's left after the loan, taxes and insurance. */
  net: number;
}

/** Monthly rent, loan payment, taxes and insurance, from yearly and
 * monthly figures in cents. Missing numbers count as nothing. */
export function monthlyPicture(i: {
  rentMonthly?: number | null;
  loanMonthly?: number | null;
  taxYearly?: number | null;
  insuranceYearly?: number | null;
}): Monthly {
  const rent = i.rentMonthly ?? 0;
  const loan = i.loanMonthly ?? 0;
  const taxes = Math.round((i.taxYearly ?? 0) / 12);
  const insurance = Math.round((i.insuranceYearly ?? 0) / 12);
  return { rent, loan, taxes, insurance, net: rent - loan - taxes - insurance };
}

/** Gross yield: a year's rent over what it's worth, as a percent with one
 * decimal. Nothing when either figure is missing. */
export function grossYieldPct(
  rentMonthly: number | null | undefined,
  value: number | null | undefined
): number | null {
  if (!rentMonthly || !value || value <= 0) return null;
  return Math.round(((rentMonthly * 12) / value) * 1000) / 10;
}

/** Value per square foot, whole dollars (inputs in cents). */
export function perSqft(
  value: number | null | undefined,
  sqft: number | null | undefined
): number | null {
  if (!value || !sqft || sqft <= 0) return null;
  return Math.round(value / 100 / sqft);
}

/** $2,197,800 → "$2.2M"; $14,800 → "$14.8K"; $950 → "$950" (input in cents). */
export function compactUsd(cents: number): string {
  const d = cents / 100;
  const a = Math.abs(d);
  if (a >= 1_000_000)
    return `$${(d / 1_000_000).toFixed(a >= 10_000_000 ? 0 : 1)}M`;
  if (a >= 10_000) return `$${Math.round(d / 1000)}K`;
  if (a >= 1000) return `$${(d / 1000).toFixed(1)}K`;
  return `$${Math.round(d)}`;
}
