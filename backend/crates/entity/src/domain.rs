//! A **domain** maps an inbound `Host` header to a tenant **and an audience**.
//! One tenant can map many domains — e.g. `app.firm.com` (admin),
//! `owners.firm.com` (owner portal), `pay.firm.com` (renter portal) — all the
//! same tenant, different audiences. Custom domains carry a `verification_token`
//! (TXT record) and a `tls_status`. The Rocket resolution guard reads `Host`,
//! looks the row up, sets `app.tenant_id` for RLS, and attaches audience + theme.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "domain")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// The fully-qualified host (e.g. `buckeye.acrenexus.com` or `portal.firm.com`).
    #[sea_orm(unique)]
    pub hostname: String,
    /// `subdomain` (acrenexus.com child) | `custom` (the firm's own domain).
    pub kind: String,
    /// `admin` | `owner` | `renter` — which app surface this host serves.
    pub audience: String,
    /// TXT record value the firm must publish to prove control of a custom domain.
    pub verification_token: Option<String>,
    /// Set once DNS verification succeeds; `None` while unverified.
    pub verified_at: Option<DateTimeWithTimeZone>,
    /// `pending` | `active` | `failed` — TLS certificate provisioning status.
    pub tls_status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
