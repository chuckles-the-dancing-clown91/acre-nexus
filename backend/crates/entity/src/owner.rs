//! An **owner** is an investor / member the firm tracks. The firm itself can be
//! an owner (`kind = "firm"`); external investors are individuals or companies.
//! Owners hold stakes in legal entities via [`crate::entity_ownership`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "owner")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `firm` | `individual` | `company`.
    pub kind: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    /// The login this owner signs into the owner portal with, once invited.
    pub user_id: Option<Uuid>,
    /// Their own spend limit; `None` uses the workspace's.
    pub approval_limit_cents: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
