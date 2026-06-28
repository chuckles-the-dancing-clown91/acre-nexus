//! A durable **record of an email the platform sent** (or simulated sending). The
//! `auto_email` background job renders an LLC template, dispatches it through the
//! configured email provider, and writes one of these rows — so there is an
//! auditable trail of what went to which tenant, and an attachment link to the
//! generated lease/letter PDF when applicable.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sent_email")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// The LLC whose branding/template produced this email, if any.
    pub llc_id: Option<Uuid>,
    pub to_address: String,
    pub cc: Option<String>,
    pub subject: String,
    /// Rendered body (text/HTML) as actually sent.
    pub body: String,
    pub template_id: Option<Uuid>,
    /// Which provider handled it: `log` (simulated) | `smtp`.
    pub provider: String,
    /// `sent` | `simulated` | `failed`.
    pub status: String,
    pub error: Option<String>,
    /// The background job that dispatched this, if sent asynchronously.
    pub job_id: Option<Uuid>,
    /// A `generated_document` attached to the email (e.g. the lease PDF), if any.
    pub generated_document_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
