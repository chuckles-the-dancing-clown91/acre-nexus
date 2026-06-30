//! Assigns a [`crate::role`] to a [`crate::user`] within a tenant context — the
//! tenancy spec's **scoped role assignment**. `tenant_id` is carried so a user
//! could (in principle) hold different roles in different workspaces. `scope`
//! together with `scope_ref_id` narrows a grant below the whole tenant (to one
//! LLC, portfolio, or property); coverage is resolved by `rbac::scope_covers`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_role")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    pub user_id: Uuid,
    pub role_id: Uuid,
    /// `NULL` when the assignment is platform-wide (staff).
    pub tenant_id: Option<Uuid>,
    /// Coverage scope: `platform` | `tenant` | `entity` | `portfolio` | `property`.
    pub scope: String,
    /// The entity/portfolio/property id when `scope` is narrower than `tenant`.
    pub scope_ref_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
