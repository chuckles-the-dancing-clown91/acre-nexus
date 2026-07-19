//! A **federated identity** links an OAuth/OIDC provider account to an
//! [`super::user`] (issue #63) — the join that makes "Log in with Google /
//! Microsoft / Apple" resolve onto the existing login record. A `(provider,
//! subject)` pair is globally unique; one user may link several providers.
//! Keyed by `user_id` with no `tenant_id` (like [`super::refresh_token`]) so it
//! is readable during login, before any tenant context is set.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "federated_identity")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    /// `google` | `microsoft` | `apple`.
    pub provider: String,
    /// The provider's stable subject identifier (OIDC `sub`).
    pub subject: String,
    /// The email the provider asserted at link time.
    pub email: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
