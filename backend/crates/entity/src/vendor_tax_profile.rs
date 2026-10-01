//! A vendor's W-9: who they are for tax purposes and their taxpayer id
//! (encrypted; only the last four are ever shown).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vendor_tax_profile")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub counterparty_id: Uuid,
    /// The name on their tax return (W-9 line 1).
    pub legal_name: String,
    /// Business or "doing business as" name (W-9 line 2).
    pub business_name: Option<String>,
    /// `individual` | `c_corp` | `s_corp` | `partnership` | `trust` | `llc_c` |
    /// `llc_s` | `llc_p` | `other`.
    pub classification: String,
    /// `ssn` | `ein`.
    pub tin_type: String,
    #[serde(skip_serializing)]
    pub tin_ciphertext: String,
    #[serde(skip_serializing)]
    pub tin_nonce: String,
    pub tin_last4: String,
    /// The date on the signed form.
    pub signed_on: Option<String>,
    /// The scanned W-9, when uploaded.
    pub document_id: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
