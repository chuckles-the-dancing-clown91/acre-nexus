//! Assigns a [`super::role`] to a [`super::user`] within a tenant context.
//! `tenant_id` is carried so a user could (in principle) hold different roles
//! in different workspaces.

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
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
