// Shared types mirroring the Rust API DTOs.

export interface Listing {
  id: string;
  title: string;
  address: string;
  city: string;
  beds: number;
  baths: number;
  sqft: number;
  rent_cents: number;
  rent_label: string;
  status: string;
  available_on: string;
  description: string;
}

export interface PublicTheme {
  company_name: string;
  logo_url: string | null;
  primary_color: string;
  accent_color: string;
  default_mode: string;
}

/**
 * A user's membership in a scope/tenant under a given persona, as returned by
 * `/auth/me`. Mirrors the membership rows surfaced for the active session.
 */
export interface Membership {
  scope: "platform" | "tenant";
  tenant_id: string | null;
  tenant_slug: string | null;
  tenant_name: string | null;
  profile_type: string;
  title: string | null;
  status: string;
  is_primary: boolean;
}

/** A workspace the current user can switch into (Acre HQ or a client tenant). */
export interface Workspace {
  kind: "platform" | "tenant";
  tenant_id: string | null;
  slug: string | null;
  name: string;
}

export interface User {
  id: string;
  email: string;
  name: string;
  tenant_id: string | null;
  is_platform_staff: boolean;
  permissions: string[];
  /** The tenant the session is currently acting in; null = Acre HQ / platform. */
  active_tenant_id: string | null;
  /** Every membership the user holds across platform + tenants. */
  memberships: Membership[];
  /** Workspaces the user can switch between. */
  workspaces: Workspace[];
}

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
  user: User;
}

export interface Property {
  id: string;
  name: string;
  address: string;
  city: string;
  llc_id: string | null;
  units: number;
  occupied_units: number;
  occupancy: string;
  monthly_rent_cents: number;
  monthly_rent_label: string;
  status: string;
  year_built: number;
  manager: string;
}

export interface CostLine {
  label: string;
  amount_cents: number;
  amount_label: string;
}

export interface PropertyProfile extends Property {
  kpis: CostLine[];
  cost_breakdown: CostLine[];
  net_revenue_cents: number;
  net_revenue_label: string;
}

export interface Kpi {
  label: string;
  value: string;
}

export interface PortfolioSummary {
  properties: number;
  units: number;
  occupied_units: number;
  occupancy_pct: number;
  monthly_revenue_cents: number;
  kpis: Kpi[];
}

export interface LlcGroup {
  id: string;
  name: string;
  ein: string;
  state: string;
  property_count: number;
  units: number;
  monthly_rent_cents: number;
  monthly_rent_label: string;
  properties: Property[];
}

// ---- Property intelligence (enrichment) ------------------------------------

export interface PropertyDetail {
  property_id: string;
  beds: number | null;
  baths: number | null;
  sqft: number | null;
  lot_size_sqft: number | null;
  property_type: string | null;
  stories: number | null;
  parking_spaces: number | null;
  heating: string | null;
  cooling: string | null;
  latitude: number | null;
  longitude: number | null;
  geocode_accuracy: string | null;
  matched_address: string | null;
  apn: string | null;
  legal_description: string | null;
  zoning: string | null;
  subdivision: string | null;
  county: string | null;
  fips: string | null;
  owner_of_record: string | null;
  last_sale_date: string | null;
  last_sale_price_cents: number | null;
  last_sale_price_label: string | null;
  flood_zone: string | null;
  walk_score: number | null;
  last_enriched_at: string | null;
}

export interface PropertyTax {
  tax_year: number;
  assessed_value_cents: number | null;
  assessed_value_label: string | null;
  tax_amount_cents: number | null;
  tax_amount_label: string | null;
  tax_rate_bps: number | null;
  source: string;
}

export interface PropertyValuation {
  as_of: string;
  estimated_value_cents: number | null;
  estimated_value_label: string | null;
  value_low_cents: number | null;
  value_high_cents: number | null;
  estimated_rent_cents: number | null;
  estimated_rent_label: string | null;
  confidence: number | null;
  source: string;
}

export interface PropertySchool {
  name: string;
  level: string;
  district: string | null;
  rating: number | null;
  distance_mi: number | null;
  grades: string | null;
}

export interface PropertyUtility {
  utility_type: string;
  provider: string;
  est_monthly_cost_cents: number | null;
  est_monthly_cost_label: string | null;
  phone: string | null;
}

export interface PropertyIntel {
  detail: PropertyDetail | null;
  valuations: PropertyValuation[];
  taxes: PropertyTax[];
  schools: PropertySchool[];
  utilities: PropertyUtility[];
}

export interface EnrichmentRun {
  id: string;
  source: string;
  status: string;
  provider: string;
  job_id: string | null;
  detail: unknown | null;
  created_at: string;
}

export interface EnrichResponse {
  job_id: string;
  scheduled: string[];
}

export interface Application {
  id: string;
  listing_id: string | null;
  applicant_name: string;
  email: string;
  phone: string;
  annual_income_label: string;
  credit_score: number | null;
  status: string;
  move_in: string;
}

export interface ApplyResponse {
  application_id: string;
  status: string;
  screening_job_id: string;
  message: string;
}
