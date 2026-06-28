//! A **user profile** holds a person's identity details, 1:1 with [`super::user`].
//! It is deliberately separate from the login record so that authentication
//! (email/username/password) is decoupled from personal data.
//!
//! ## Sensitive fields
//! SSN and government-ID numbers are **never stored in clear**. They are
//! encrypted with AES-256-GCM (`api::pii`) and persisted as base64 ciphertext +
//! nonce; only the last four digits are kept in clear for display. The
//! ciphertext/nonce columns are `skip_serializing` so they can never leak
//! through an API response — decryption is an explicit, permission-gated action.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_profile")]
pub struct Model {
    /// Primary key and FK to `app_user.id` (1:1).
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    pub legal_first_name: Option<String>,
    pub legal_middle_name: Option<String>,
    pub legal_last_name: Option<String>,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<Date>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,

    // --- Sensitive (encrypted at rest) ---
    #[serde(skip_serializing)]
    pub ssn_ciphertext: Option<String>,
    #[serde(skip_serializing)]
    pub ssn_nonce: Option<String>,
    /// Last 4 of the SSN, safe to display.
    pub ssn_last4: Option<String>,
    /// e.g. `drivers_license`, `passport`.
    pub gov_id_type: Option<String>,
    #[serde(skip_serializing)]
    pub gov_id_ciphertext: Option<String>,
    #[serde(skip_serializing)]
    pub gov_id_nonce: Option<String>,
    pub gov_id_last4: Option<String>,

    pub photo_url: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
