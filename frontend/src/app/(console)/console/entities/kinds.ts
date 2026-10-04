// The counterparty kinds offered in selects, in display order.

export const KINDS = [
  "bank",
  "lender",
  "insurer",
  "title",
  "contractor",
  "inspector",
  "appraiser",
  "attorney",
  "property_manager",
  "utility",
  "other",
] as const;

/** `property_manager` → `Property manager`. */
export function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}
