// What a property is decides how it's rented and what the console offers for
// it. Mirrors `property_kind.rs` on the server.

export type UnitMode = "single" | "multi" | "sites" | "none";

export interface Kind {
  key: string;
  label: string;
  /** How people say it in a sentence. */
  noun: string;
  mode: UnitMode;
  /** What a rentable space is called here. */
  unit: string;
  units: string;
}

export const KINDS: Kind[] = [
  {
    key: "single_family",
    label: "Single-family home",
    noun: "house",
    mode: "single",
    unit: "Home",
    units: "Home",
  },
  {
    key: "townhome",
    label: "Townhome",
    noun: "townhome",
    mode: "single",
    unit: "Home",
    units: "Home",
  },
  {
    key: "condo",
    label: "Condo",
    noun: "condo",
    mode: "single",
    unit: "Home",
    units: "Home",
  },
  {
    key: "manufactured",
    label: "Manufactured home",
    noun: "home",
    mode: "single",
    unit: "Home",
    units: "Home",
  },
  {
    key: "multi_family",
    label: "Apartments",
    noun: "building",
    mode: "multi",
    unit: "Unit",
    units: "Units",
  },
  {
    key: "commercial",
    label: "Commercial",
    noun: "building",
    mode: "multi",
    unit: "Suite",
    units: "Suites",
  },
  {
    key: "campground",
    label: "Campground",
    noun: "campground",
    mode: "sites",
    unit: "Site",
    units: "Sites",
  },
  {
    key: "rv_park",
    label: "RV park",
    noun: "RV park",
    mode: "sites",
    unit: "Site",
    units: "Sites",
  },
  {
    key: "land",
    label: "Land",
    noun: "parcel",
    mode: "none",
    unit: "Parcel",
    units: "Parcels",
  },
];

const ALIASES: Record<string, string> = {
  singlefamily: "single_family",
  house: "single_family",
  sfh: "single_family",
  townhouse: "townhome",
  condominium: "condo",
  mobilehome: "manufactured",
  multifamily: "multi_family",
  apartment: "multi_family",
  apartments: "multi_family",
  duplex: "multi_family",
  office: "commercial",
  retail: "commercial",
  camp: "campground",
  rvpark: "rv_park",
  lot: "land",
  acreage: "land",
};

/** A stored or typed spelling as one of the known kinds, if it is one. */
export function normalizeKind(raw: string | null | undefined): Kind | null {
  const k = (raw ?? "").toLowerCase().replace(/[^a-z0-9]/g, "");
  if (!k) return null;
  const key =
    ALIASES[k] ?? KINDS.find((x) => x.key.replace(/_/g, "") === k)?.key;
  return KINDS.find((x) => x.key === key) ?? null;
}

/** The kind of a property whose type may be blank: its type, else guessed from
 * how many units it has. */
export function kindOf(
  p: { property_type?: string | null; units?: number } | null | undefined
): Kind {
  return (
    normalizeKind(p?.property_type) ??
    ((p?.units ?? 0) > 1
      ? KINDS.find((k) => k.key === "multi_family")!
      : KINDS.find((k) => k.key === "single_family")!)
  );
}

/** Which property page tabs a kind offers, in order. */
export function tabsFor(kind: Kind): string[] {
  const base = ["overview"];
  if (kind.mode === "multi") base.push("units");
  if (kind.mode !== "none") base.push("systems", "meters");
  base.push("parcel", "permits", "history", "schools", "insurance");
  return base;
}
