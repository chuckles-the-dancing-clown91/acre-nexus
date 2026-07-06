use crate::dto::usd;
use crate::routes::banking::dto::BankAccountResp;
use crate::routes::mortgages::dto::MortgageDto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyResp {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub city: String,
    pub llc_id: Option<Uuid>,
    pub portfolio_id: Option<Uuid>,
    pub units: i32,
    pub occupied_units: i32,
    pub occupancy: String,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    pub status: String,
    pub year_built: i32,
    pub manager: String,
    pub property_type: String,
    pub strategy: String,
    pub workflow_stage: String,
    pub purchase_price_cents: Option<i64>,
    pub acquired_on: Option<String>,
    /// Hero photo shown top-left on the profile.
    pub image_url: Option<String>,
}

impl From<entity::property::Model> for PropertyResp {
    fn from(p: entity::property::Model) -> Self {
        PropertyResp {
            occupancy: format!("{}/{}", p.occupied_units, p.units),
            monthly_rent_label: usd(p.monthly_rent_cents),
            id: p.id,
            name: p.name,
            address: p.address,
            city: p.city,
            llc_id: p.llc_id,
            portfolio_id: p.portfolio_id,
            units: p.units,
            occupied_units: p.occupied_units,
            monthly_rent_cents: p.monthly_rent_cents,
            status: p.status,
            year_built: p.year_built,
            manager: p.manager,
            property_type: p.property_type,
            strategy: p.strategy,
            workflow_stage: p.workflow_stage,
            purchase_price_cents: p.purchase_price_cents,
            acquired_on: p.acquired_on,
            image_url: p.image_url,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreatePropertyReq {
    pub name: String,
    pub address: String,
    pub city: String,
    pub llc_id: Option<Uuid>,
    pub portfolio_id: Option<Uuid>,
    pub units: i32,
    pub occupied_units: i32,
    pub monthly_rent_cents: i64,
    pub status: Option<String>,
    pub year_built: Option<i32>,
    pub manager: Option<String>,
    pub property_type: Option<String>,
    pub strategy: Option<String>,
    /// Hero photo URL for the property profile.
    pub image_url: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CostLine {
    pub label: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

/// The physical breakdown of the home, shown beside the hero image. Merges the
/// property's own fields with the enrichment engine's `property_detail`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct HomeBreakdown {
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub lot_size_sqft: Option<i64>,
    pub stories: Option<i32>,
    pub parking_spaces: Option<i32>,
    pub heating: Option<String>,
    pub cooling: Option<String>,
    pub year_built: Option<i32>,
    /// Best-known type: the enriched `property_detail.property_type` if set,
    /// else the investor-entered `property.property_type`.
    pub property_type: Option<String>,
}

/// The address plus how confidently it has been verified/geocoded.
#[derive(Serialize, schemars::JsonSchema)]
pub struct AddressStatus {
    pub address: String,
    pub city: String,
    /// The normalized address the geocoder matched, if enrichment has run.
    pub matched_address: Option<String>,
    /// Match-quality label from the geocoder (e.g. `Exact`).
    pub geocode_accuracy: Option<String>,
    /// True once the address has been geocoded (lat/long present).
    pub verified: bool,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub county: Option<String>,
    pub apn: Option<String>,
}

/// One active tenancy, summarised for the rental-status header.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ActiveLeaseSummary {
    pub lease_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub tenant_name: String,
    pub rent_cents: i64,
    pub rent_label: String,
    /// `upcoming` | `active` | `notice` | `expired` | `ended`.
    pub status: String,
    /// `current` | `late` | `partial`.
    pub payment_status: String,
    pub balance_cents: i64,
    pub balance_label: String,
}

/// The rental picture: occupancy, and the current tenancies with their standing.
#[derive(Serialize, schemars::JsonSchema)]
pub struct RentalStatus {
    /// `Stabilized` | `Vacant` | `Lease-up` | `Renovating` (the property's status).
    pub status: String,
    pub occupancy: String,
    pub units: i32,
    pub occupied_units: i32,
    pub vacant_units: i32,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    /// Count of active leases whose resident is behind (`late`/`partial`).
    pub delinquent_leases: i32,
    pub active_leases: Vec<ActiveLeaseSummary>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyProfileResp {
    #[serde(flatten)]
    pub property: PropertyResp,
    /// Hero photo shown top-left (mirrors `property.image_url` for convenience).
    pub image_url: Option<String>,
    /// Physical breakdown of the home, shown to the right of the image.
    pub home: HomeBreakdown,
    /// Address + geocode/verification status.
    pub address_status: AddressStatus,
    /// Occupancy + current tenancies and their payment standing.
    pub rental_status: RentalStatus,
    pub kpis: Vec<CostLine>,
    pub cost_breakdown: Vec<CostLine>,
    pub net_revenue_cents: i64,
    pub net_revenue_label: String,
    /// Whether the property carries any financing.
    pub financed: bool,
    /// Total monthly debt service (sum of mortgage payments), in cents.
    pub debt_service_cents: i64,
    pub debt_service_label: String,
    /// Levered cash flow: net operating income − debt service.
    pub cash_flow_cents: i64,
    pub cash_flow_label: String,
    /// Sum of outstanding loan balances, in cents.
    pub total_loan_balance_cents: i64,
    pub total_loan_balance_label: String,
    /// Estimated equity: best-known value − loan balances, in cents.
    pub equity_cents: i64,
    pub equity_label: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdatePropertyReq {
    pub name: Option<String>,
    pub status: Option<String>,
    pub occupied_units: Option<i32>,
    pub monthly_rent_cents: Option<i64>,
    pub manager: Option<String>,
    /// Set/replace the hero photo URL. Pass an empty string to clear it.
    pub image_url: Option<String>,
}

/// The bank/lender that owns a loan, plus the contact there — resolved from the
/// entities registry via the mortgage's `lender_id`.
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct LenderContact {
    pub id: Uuid,
    pub name: String,
    /// Usually `bank` or `lender`.
    pub kind: String,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
}

impl From<entity::counterparty::Model> for LenderContact {
    fn from(c: entity::counterparty::Model) -> Self {
        LenderContact {
            id: c.id,
            name: c.name,
            kind: c.kind,
            contact_name: c.contact_name,
            email: c.email,
            phone: c.phone,
            website: c.website,
            address: c.address,
        }
    }
}

/// A loan on the property with its owning bank + contact resolved inline.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LoanDto {
    #[serde(flatten)]
    pub loan: MortgageDto,
    /// The bank that owns the loan and the contact there (`None` if the loan is
    /// not yet linked to an entity).
    pub lender: Option<LenderContact>,
}

/// The Financials tab: levered economics, loans (each with its bank + contact),
/// and the owning legal entity's bank accounts.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyFinancialsResp {
    pub property_id: Uuid,
    pub financed: bool,
    pub net_revenue_cents: i64,
    pub net_revenue_label: String,
    pub debt_service_cents: i64,
    pub debt_service_label: String,
    pub cash_flow_cents: i64,
    pub cash_flow_label: String,
    pub total_loan_balance_cents: i64,
    pub total_loan_balance_label: String,
    pub equity_cents: i64,
    pub equity_label: String,
    pub cost_breakdown: Vec<CostLine>,
    /// Loans secured against the property, ordered by lien position.
    pub loans: Vec<LoanDto>,
    /// The owning LLC's operating/trust bank accounts (empty if no LLC linked).
    pub bank_accounts: Vec<BankAccountResp>,
}
