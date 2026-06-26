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
