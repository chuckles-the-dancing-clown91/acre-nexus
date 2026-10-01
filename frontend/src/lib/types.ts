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
  /** ISO time it was listed. */
  listed_at?: string;
  /** Pictures in order; the first is the hero. */
  photos?: ListingPhoto[];
}

export interface ListingPhoto {
  url: string;
  alt: string;
  caption: string | null;
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

/** A workspace the current user can switch into (Vantedge HQ or a client tenant). */
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
  /** The tenant the session is currently acting in; null = Vantedge HQ / platform. */
  active_tenant_id: string | null;
  /** Every membership the user holds across platform + tenants. */
  memberships: Membership[];
  /** Workspaces the user can switch between. */
  workspaces: Workspace[];
  /**
   * How much of the active workspace the user sees: the whole company, or
   * only the properties assigned to them (field roles and owners).
   */
  reach?: Reach;
}

export interface Reach {
  scope: "company" | "properties";
  property_ids: string[];
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
  state: string;
  postal_code: string;
  /** `none` | `stored` | `placeholder` | `failed` — the fetched street photo. */
  photo_status: string;
  photo_error: string | null;
  llc_id: string | null;
  units: number;
  occupied_units: number;
  occupancy: string;
  monthly_rent_cents: number;
  monthly_rent_label: string;
  status: string;
  year_built: number;
  manager: string;
  property_type: string;
  strategy: string;
  workflow_stage: string;
  purchase_price_cents: number | null;
  acquired_on: string | null;
  image_url: string | null;
}

export interface CostLine {
  label: string;
  amount_cents: number;
  amount_label: string;
}

// ---- Property profile header dossier ---------------------------------------

export interface HomeBreakdown {
  beds: number | null;
  baths: number | null;
  sqft: number | null;
  lot_size_sqft: number | null;
  stories: number | null;
  parking_spaces: number | null;
  heating: string | null;
  cooling: string | null;
  year_built: number | null;
  property_type: string | null;
}

export interface AddressStatus {
  address: string;
  city: string;
  matched_address: string | null;
  geocode_accuracy: string | null;
  verified: boolean;
  latitude: number | null;
  longitude: number | null;
  county: string | null;
  apn: string | null;
}

export interface ActiveLeaseSummary {
  lease_id: string;
  unit_id: string | null;
  tenant_name: string;
  rent_cents: number;
  rent_label: string;
  status: string;
  payment_status: string;
  balance_cents: number;
  balance_label: string;
}

export interface RentalStatus {
  status: string;
  occupancy: string;
  units: number;
  occupied_units: number;
  vacant_units: number;
  monthly_rent_cents: number;
  monthly_rent_label: string;
  delinquent_leases: number;
  active_leases: ActiveLeaseSummary[];
}

export interface PropertyProfile extends Property {
  image_url: string | null;
  home: HomeBreakdown;
  address_status: AddressStatus;
  rental_status: RentalStatus;
  kpis: CostLine[];
  cost_breakdown: CostLine[];
  net_revenue_cents: number;
  net_revenue_label: string;
  financed: boolean;
  debt_service_cents: number;
  debt_service_label: string;
  cash_flow_cents: number;
  cash_flow_label: string;
  total_loan_balance_cents: number;
  total_loan_balance_label: string;
  equity_cents: number;
  equity_label: string;
}

// ---- Financials tab --------------------------------------------------------

export interface BankAccount {
  id: string;
  entity_id: string;
  kind: string;
  institution: string;
  masked_number: string | null;
  status: string;
  provider: string | null;
  linked: boolean;
  last_synced_at: string | null;
}

/** The bank that owns a loan + the contact there (resolved via `lender_id`). */
export interface LenderContact {
  id: string;
  name: string;
  kind: string;
  contact_name: string | null;
  email: string | null;
  phone: string | null;
  website: string | null;
  address: string | null;
}

/** A mortgage with its owning bank + contact flattened in. */
export interface Loan extends Mortgage {
  lender: LenderContact | null;
}

export interface PropertyFinancials {
  property_id: string;
  financed: boolean;
  net_revenue_cents: number;
  net_revenue_label: string;
  debt_service_cents: number;
  debt_service_label: string;
  cash_flow_cents: number;
  cash_flow_label: string;
  total_loan_balance_cents: number;
  total_loan_balance_label: string;
  equity_cents: number;
  equity_label: string;
  cost_breakdown: CostLine[];
  loans: Loan[];
  bank_accounts: BankAccount[];
}

// ---- Maintenance tab -------------------------------------------------------

export interface PropertyMaintenance {
  property_id: string;
  total_count: number;
  open_count: number;
  open_cost_cents: number;
  open_cost_label: string;
  open: MaintenanceTicket[];
  history: MaintenanceTicket[];
  /** Recorded cost of resolved work, all time / last 12 months. */
  history_cost_cents: number;
  history_cost_label: string;
  last_12mo_cents: number;
  /** Where the money went: cost by ticket category. */
  by_category: {
    category: string;
    tickets: number;
    cents: number;
    label: string;
  }[];
  /** Expenses booked against this property. */
  expenses_cents: number;
  assets: Asset[];
  plans: MaintenancePlan[];
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
  open_tickets: number | null;
  urgent_tickets: number | null;
  delinquent_tenants: number | null;
  delinquent_balance_cents: number | null;
  delinquent_balance_label: string | null;
  pending_applications: number | null;
  upcoming_reminders: number | null;
  overdue_reminders: number | null;
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

// ---- Entities registry (counterparties) ------------------------------------

export interface Counterparty {
  id: string;
  kind: string;
  name: string;
  contact_name: string | null;
  email: string | null;
  phone: string | null;
  website: string | null;
  address: string | null;
  notes: string | null;
  /** Linked partner system (`alpha`), when this vendor runs one. */
  partner_kind: string | null;
  partner_status: string | null;
  created_at: string;
  updated_at: string;
}

export interface CounterpartyNote {
  id: string;
  counterparty_id: string;
  author_user_id: string | null;
  body: string;
  created_at: string;
}

export interface CounterpartyDetail extends Counterparty {
  notes_log: CounterpartyNote[];
}

export interface CreateCounterpartyInput {
  kind: string;
  name: string;
  contact_name?: string;
  email?: string;
  phone?: string;
  website?: string;
  address?: string;
  notes?: string;
}

// ---- Financing (mortgages) -------------------------------------------------

export interface Mortgage {
  id: string;
  property_id: string;
  lender_id: string | null;
  kind: string;
  position: number;
  original_amount_cents: number | null;
  original_amount_label: string | null;
  current_balance_cents: number | null;
  current_balance_label: string | null;
  interest_rate_bps: number | null;
  interest_rate_pct: number | null;
  term_months: number | null;
  monthly_payment_cents: number | null;
  monthly_payment_label: string | null;
  escrow_monthly_cents: number | null;
  escrow_monthly_label: string | null;
  start_date: string | null;
  maturity_date: string | null;
  loan_number: string | null;
  status: string;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateMortgageInput {
  lender_id?: string;
  kind: string;
  position?: number;
  original_amount_cents?: number;
  current_balance_cents?: number;
  interest_rate_bps?: number;
  term_months?: number;
  monthly_payment_cents?: number;
  escrow_monthly_cents?: number;
  start_date?: string;
  maturity_date?: string;
  loan_number?: string;
}

// ---- Workflows -------------------------------------------------------------

export interface WorkflowStage {
  key: string;
  label: string;
  reached: boolean;
  current: boolean;
}

export interface WorkflowEvent {
  id: string;
  strategy: string;
  from_stage: string | null;
  to_stage: string;
  note: string | null;
  actor_user_id: string | null;
  /** Display name of the actor; null for automated transitions. */
  actor_name: string | null;
  created_at: string;
}

export interface Workflow {
  strategy: string;
  strategy_label: string;
  strategy_description: string;
  current_stage: string;
  stages: WorkflowStage[];
  history: WorkflowEvent[];
}

// ---- Onboarding ------------------------------------------------------------

export interface OnboardMortgageInput {
  lender_id?: string;
  lender_name?: string;
  kind: string;
  position?: number;
  original_amount_cents?: number;
  current_balance_cents?: number;
  interest_rate_bps?: number;
  term_months?: number;
  monthly_payment_cents?: number;
  escrow_monthly_cents?: number;
  start_date?: string;
  maturity_date?: string;
  loan_number?: string;
}

export interface OnboardInput {
  name: string;
  address: string;
  city: string;
  state?: string;
  postal_code?: string;
  llc_id?: string;
  units?: number;
  occupied_units?: number;
  monthly_rent_cents?: number;
  year_built?: number;
  manager?: string;
  status?: string;
  property_type: string;
  strategy: string;
  purchase_price_cents?: number;
  acquired_on?: string;
  mortgages: OnboardMortgageInput[];
  assignments?: CreateAssignmentInput[];
  enrich: boolean;
}

export interface OnboardResponse {
  property_id: string;
  strategy: string;
  workflow_stage: string;
  mortgages_created: number;
  lenders_created: number;
  assignments_created: number;
  enrich_job_id: string | null;
}

// ---- Staff assignments (property / LLC) -----------------------------------

/** The subject an assignment targets. */
export type AssignmentSubject = "property" | "entity";

/** Relationships a person can be assigned as. Each maps to a tenant role that is
 * granted at the subject's scope, so assigning also confers access. */
export const ASSIGNABLE_RELATIONSHIPS: { key: string; label: string }[] = [
  { key: "property_manager", label: "Property Manager" },
  { key: "landlord", label: "Landlord / Owner" },
  { key: "maintenance", label: "Maintenance" },
  { key: "leasing_agent", label: "Leasing Agent" },
  { key: "back_office", label: "Back-office Staff" },
];

export interface Assignment {
  id: string;
  subject_type: AssignmentSubject;
  subject_id: string;
  user_id: string;
  user_name: string;
  user_email: string;
  relationship: string;
  relationship_label: string;
  role_id: string | null;
  is_primary: boolean;
  title: string | null;
  notes: string | null;
  created_at: string;
}

export interface CreateAssignmentInput {
  user_id: string;
  relationship: string;
  is_primary?: boolean;
  title?: string;
  notes?: string;
}

// ---- System settings ------------------------------------------------------

/** One setting merged with its catalog metadata + the tenant's value. */
export interface SettingView {
  key: string;
  label: string;
  description: string;
  group: string;
  /** "bool" | "int" | "text". */
  kind: string;
  value: unknown;
  default: unknown;
}

// ---- Application workflow --------------------------------------------------

export interface AppWorkflowStage {
  key: string;
  label: string;
  terminal: boolean;
  reached: boolean;
  current: boolean;
}

export interface ApplicationEvent {
  id: string;
  from_status: string | null;
  to_status: string;
  note: string | null;
  actor_user_id: string | null;
  created_at: string;
}

export interface ApplicationWorkflow {
  current_status: string;
  stages: AppWorkflowStage[];
  offramps: AppWorkflowStage[];
  allowed_next: string[];
  history: ApplicationEvent[];
}

export interface AppWorkflowCatalogStage {
  key: string;
  label: string;
  terminal: boolean;
  transitions: string[];
}

export interface AppWorkflowCatalog {
  stages: AppWorkflowCatalogStage[];
  offramps: AppWorkflowCatalogStage[];
}

// ---- Rentals: units, leases, payments -------------------------------------

export interface Unit {
  id: string;
  property_id: string;
  unit_number: string;
  beds: number | null;
  baths: number | null;
  sqft: number | null;
  market_rent_cents: number | null;
  market_rent_label: string | null;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface CreateUnitInput {
  unit_number: string;
  beds?: number;
  baths?: number;
  sqft?: number;
  market_rent_cents?: number;
  status?: string;
}

export interface Lease {
  id: string;
  property_id: string;
  unit_id: string | null;
  tenant_name: string;
  tenant_email: string | null;
  tenant_phone: string | null;
  rent_cents: number;
  rent_label: string;
  deposit_cents: number | null;
  deposit_label: string | null;
  start_date: string;
  end_date: string | null;
  status: string;
  payment_status: string;
  balance_cents: number;
  application_id: string | null;
  has_pet: boolean;
  pet_details: string | null;
  is_military: boolean;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface LeasePayment {
  id: string;
  lease_id: string;
  due_date: string;
  amount_cents: number;
  amount_label: string;
  paid_date: string | null;
  status: string;
  method: string | null;
  created_at: string;
}

export interface LeaseDetail extends Lease {
  payments: LeasePayment[];
}

export interface CreateLeaseInput {
  unit_id?: string;
  tenant_name: string;
  tenant_email?: string;
  tenant_phone?: string;
  rent_cents: number;
  deposit_cents?: number;
  start_date: string;
  end_date?: string;
  status?: string;
  payment_status?: string;
  notes?: string;
}

export interface RecordPaymentInput {
  due_date: string;
  amount_cents: number;
  paid_date?: string;
  status?: string;
  method?: string;
}

// ---- Maintenance: tickets --------------------------------------------------

export interface MaintenanceTicket {
  id: string;
  property_id: string;
  unit_id: string | null;
  lease_id: string | null;
  title: string;
  description: string | null;
  category: string;
  priority: string;
  status: string;
  assignee_user_id: string | null;
  assignee_entity_id: string | null;
  reporter: string | null;
  /** Where in the home (e.g. "Kitchen"). */
  location: string | null;
  /** Entry instructions. */
  access_notes: string | null;
  /** Entry authorized when the resident is out. */
  permission_to_enter: boolean;
  /** Registered equipment being serviced. */
  asset_id: string | null;
  /** What an on-hold ticket is blocked by. */
  waiting_on: string | null;
  /** ISO date the waiting-on follow-up is due. */
  follow_up_date: string | null;
  /** Resident feedback after resolution (1–5). */
  rating: number | null;
  review_comment: string | null;
  due_date: string | null;
  cost_cents: number | null;
  cost_label: string | null;
  /** SLA / lifecycle timestamps (Phase 6). */
  first_response_at: string | null;
  resolved_at: string | null;
  sla_response_due_at: string | null;
  sla_resolve_due_at: string | null;
  /** `none` | `on_track` | `met` | `breached`, derived server-side. */
  sla_response_state: string;
  sla_resolve_state: string;
  /** Sent to a vendor's own system (Alpha): who, their job id and status. */
  partner_counterparty_id: string | null;
  partner_job_id: string | null;
  partner_status: string | null;
  partner_synced_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface TicketQuote {
  id: string;
  ticket_id: string;
  entity_id: string;
  entity_name: string | null;
  description: string;
  amount_cents: number;
  amount_label: string;
  status: "pending" | "approved" | "rejected";
  decided_at: string | null;
  created_at: string;
}

export interface MaintenancePlan {
  id: string;
  property_id: string;
  unit_id: string | null;
  title: string;
  description: string | null;
  category: string;
  priority: string;
  /** The appliance this routine is for (its parts pre-list on each ticket). */
  asset_id: string | null;
  cadence_days: number;
  next_due_date: string;
  active: boolean;
  last_ticket_id: string | null;
  /** The job kit each routine work order starts with. */
  issue_template_id?: string | null;
  created_at: string;
}

export interface TicketComment {
  id: string;
  ticket_id: string;
  author_user_id: string | null;
  kind: string;
  /** `public` | `internal` (staff-only note). */
  visibility: "public" | "internal";
  author_name: string | null;
  body: string;
  /** Photos and files attached to the note. */
  document_ids?: string[];
  created_at: string;
}

export interface TicketDetail extends MaintenanceTicket {
  comments: TicketComment[];
  lines: TicketLine[];
  asset_name: string | null;
  quotes: TicketQuote[];
  inbound_email_address: string | null;
  /** The parts loop: potential → needed → from stock / to order → used. */
  parts: TicketPart[];
}

export type PartStatus =
  | "potential"
  | "needed"
  | "from_stock"
  | "to_order"
  | "ordered"
  | "pick_up"
  | "received"
  | "used"
  | "skipped";

/** One part on a work order, moving through the parts loop. */
export interface TicketPart {
  id: string;
  ticket_id: string;
  inventory_item_id: string | null;
  name: string;
  quantity: number;
  status: PartStatus;
  /** `asset` | `finding` | `plan` | `typed` */
  source: string;
  finding_comment_id: string | null;
  need_by: string | null;
  ship_to: "property" | "office" | "other" | null;
  ship_to_note: string | null;
  vendor: string | null;
  tracking: string | null;
  unit_cost_cents: number | null;
  note: string | null;
  /** On the shelf right now (stock items). */
  in_stock: number | null;
  ordered_at: string | null;
  received_at: string | null;
  created_at: string;
}

/** One itemized part / labor / fee entry on a work order. */
export interface TicketLine {
  id: string;
  ticket_id: string;
  kind: "part" | "labor" | "fee" | "other";
  description: string;
  inventory_item_id: string | null;
  serial_number: string | null;
  quantity: number;
  unit_cost_cents: number;
  unit_cost_label: string;
  total_cents: number;
  total_label: string;
  created_at: string;
}

/** A stockroom item the maintenance team draws from. */
export interface InventoryItem {
  id: string;
  property_id: string | null;
  name: string;
  sku: string | null;
  /** UPC / EAN or the workspace's own code — what the scanner reads. */
  barcode: string | null;
  unit: string;
  vendor: string | null;
  category: string;
  quantity: number;
  unit_cost_cents: number | null;
  unit_cost_label: string | null;
  /** Quantity × unit cost. */
  value_cents: number;
  reorder_level: number;
  low_stock: boolean;
  storage_location: string | null;
  serial_numbers: string[];
  notes: string | null;
  status: "active" | "archived";
  created_at: string;
}

/** A registered piece of serviceable equipment (AC, water heater, appliance). */
export interface Asset {
  id: string;
  property_id: string;
  unit_id: string | null;
  kind: string;
  name: string;
  make: string | null;
  model: string | null;
  serial_number: string | null;
  install_date: string | null;
  warranty_expires: string | null;
  /** `none` | `active` | `expired`, derived server-side. */
  warranty_state: string;
  location: string | null;
  purchased_on: string | null;
  purchase_price_cents: number | null;
  expected_life_years: number | null;
  /** Years of expected life left; negative = past due. */
  years_left: number | null;
  warranty_provider: string | null;
  warranty_notes: string | null;
  notes: string | null;
  status: "active" | "retired";
  created_at: string;
}

export interface CreateAssetInput {
  property_id: string;
  unit_id?: string;
  kind?: string;
  name: string;
  make?: string;
  model?: string;
  serial_number?: string;
  install_date?: string;
  warranty_expires?: string;
  location?: string;
  purchased_on?: string;
  purchase_price_cents?: number;
  expected_life_years?: number;
  warranty_provider?: string;
  warranty_notes?: string;
  notes?: string;
}

export interface UpdateAssetInput {
  kind?: string;
  name?: string;
  make?: string;
  model?: string;
  serial_number?: string;
  install_date?: string;
  warranty_expires?: string;
  location?: string;
  purchased_on?: string;
  purchase_price_cents?: number;
  expected_life_years?: number;
  warranty_provider?: string;
  warranty_notes?: string;
  notes?: string;
  status?: "active" | "retired";
}

export interface CreateTicketInput {
  title: string;
  description?: string;
  category?: string;
  priority?: string;
  unit_id?: string;
  lease_id?: string;
  assignee_user_id?: string;
  assignee_entity_id?: string;
  reporter?: string;
  due_date?: string;
  cost_cents?: number;
}

export interface UpdateTicketInput {
  /** Why to send a vendor without current insurance (when that is required). */
  coi_override_reason?: string;
  title?: string;
  description?: string;
  category?: string;
  priority?: string;
  status?: string;
  assignee_user_id?: string;
  assignee_entity_id?: string;
  reporter?: string;
  location?: string;
  access_notes?: string;
  permission_to_enter?: boolean;
  asset_id?: string;
  /** Set with status on_hold: parts|vendor|resident|owner|other ("none" clears). */
  waiting_on?: string;
  follow_up_date?: string;
  follow_up_note?: string;
  due_date?: string;
  cost_cents?: number;
}

// ---- Title: ownership + liens ----------------------------------------------

export interface Ownership {
  id: string;
  property_id: string;
  owner_kind: string;
  owner_id: string | null;
  owner_name: string;
  vesting: string | null;
  percent_bps: number;
  deed_type: string | null;
  deed_recorded_date: string | null;
  deed_reference: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateOwnershipInput {
  owner_kind?: string;
  owner_id?: string;
  owner_name: string;
  vesting?: string;
  percent_bps?: number;
  deed_type?: string;
  deed_recorded_date?: string;
  deed_reference?: string;
}

export interface Lien {
  id: string;
  property_id: string;
  lienholder_id: string | null;
  lienholder_name: string;
  kind: string;
  amount_cents: number | null;
  amount_label: string | null;
  position: number | null;
  recorded_date: string | null;
  status: string;
  reference: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateLienInput {
  lienholder_id?: string;
  lienholder_name: string;
  kind?: string;
  amount_cents?: number;
  position?: number;
  recorded_date?: string;
  status?: string;
  reference?: string;
  notes?: string;
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
  has_pet: boolean;
  pet_details: string | null;
  is_military: boolean;
  /** Intake door: public | portal | back_office. */
  source: string;
  /** Background-check outcome once screening finishes: cleared | failed. */
  screening_status: string | null;
  screened_at: string | null;
  /** When the applicant authorized the consumer report (FCRA §604(b)). */
  screening_consent_at: string | null;
  /** When the FCRA §615(a) adverse-action notice was sent, if it was. */
  adverse_action_at: string | null;
  adverse_action_document_id: string | null;
  created_at: string;
}

/** The stored screening (consumer) report for an application. */
export interface ScreeningReport {
  id: string;
  application_id: string;
  /** Provider key (checkr). */
  provider: string;
  /** pending | in_progress | complete | failed. */
  status: string;
  include_credit: boolean;
  include_criminal: boolean;
  include_eviction: boolean;
  consent_at: string | null;
  credit_score: number | null;
  criminal_records: number | null;
  eviction_records: number | null;
  /** Provider assessment: clear | consider. */
  recommendation: string | null;
  /** Policy verdict once landed: cleared | failed. */
  result: string | null;
  /** The policy checks that tripped (empty when cleared). */
  reasons: string[];
  completed_at: string | null;
  created_at: string;
}

/** A listing as the console sees it (includes visibility). */
export interface ConsoleListing {
  id: string;
  property_id: string | null;
  title: string;
  address: string;
  city: string;
  beds: number;
  baths: number;
  sqft: number;
  rent_cents: number;
  rent_label: string;
  /** Available | New | Pending | Leased. */
  status: string;
  available_on: string;
  description: string;
  is_public: boolean;
  created_at: string;
}

export interface CreateListingInput {
  title?: string;
  rent_cents: number;
  beds?: number;
  baths?: number;
  sqft?: number;
  available_on?: string;
  description?: string;
  is_public?: boolean;
}

export interface UpdateListingInput {
  title?: string;
  rent_cents?: number;
  beds?: number;
  baths?: number;
  sqft?: number;
  available_on?: string;
  description?: string;
  status?: string;
  is_public?: boolean;
}

/** Back-office application intake. */
export interface CreateApplicationInput {
  listing_id?: string;
  applicant_name: string;
  email: string;
  phone?: string;
  annual_income_cents?: number;
  credit_score?: number;
  move_in?: string;
  has_pet?: boolean;
  pet_details?: string;
  is_military?: boolean;
  /** Staff attest the applicant signed the consumer-report authorization. */
  screening_consent?: boolean;
}

/** Renter-portal application (identity comes from the account). */
export interface PortalApplyInput {
  listing_id?: string;
  applicant_name?: string;
  phone?: string;
  annual_income_cents?: number;
  credit_score?: number;
  move_in?: string;
  has_pet?: boolean;
  pet_details?: string;
  is_military?: boolean;
  /** The applicant authorizes the consumer report (FCRA) — required. */
  screening_consent?: boolean;
}

export interface ApplyResponse {
  application_id: string;
  status: string;
  screening_job_id: string;
  message: string;
}
