//! A **membership** places a [`crate::user`] into a scope with a persona.
//!
//! * **Platform** memberships (`scope = "platform"`, `tenant_id = NULL`) make a
//!   user an Acre employee.
//! * **Tenant** memberships (`scope = "tenant"`, `tenant_id = <client>`) make a
//!   user part of a client workspace as a landlord, back-office staffer,
//!   property manager, renter, etc.
//!
//! A single user may hold several memberships (e.g. an Acre support agent who is
//! also a landlord in a client workspace). The persona is the `profile_type`;
//! the actual permissions come from RBAC roles assigned via `user_role`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "membership")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    /// `platform` or `tenant`.
    pub scope: String,
    /// The client workspace for tenant memberships; `NULL` for platform.
    pub tenant_id: Option<Uuid>,
    /// Persona key — FK to `profile_type.key`.
    pub profile_type: String,
    /// Optional job title shown in the directory.
    pub title: Option<String>,
    /// `active` | `invited` | `suspended`.
    pub status: String,
    /// The user's primary membership (drives default workspace / "view as").
    pub is_primary: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::tenant::Entity",
        from = "Column::TenantId",
        to = "super::tenant::Column::Id"
    )]
    Tenant,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
