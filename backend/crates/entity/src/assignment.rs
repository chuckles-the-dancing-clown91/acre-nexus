//! A **staff assignment**: a person (property manager, landlord, maintenance,
//! leasing agent, back-office) attached to a specific property or legal entity
//! (LLC). An assignment records the working relationship *and* confers scoped
//! access — when created, a matching `user_role` grant is added at
//! `property:{id}` / `entity:{id}` scope; when removed, that grant is revoked.
//!
//! `subject_type` is `property` | `entity` (LLC) and `subject_id` points at the
//! row in that table. `relationship` is a tenant role key (e.g.
//! `property_manager`, `landlord`); `role_id` is the resolved [`crate::role`]
//! actually granted. At most one row per (subject, user, relationship).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "assignment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `property` | `entity` (LLC).
    pub subject_type: String,
    /// FK into the `property` or `llc` table, per `subject_type`.
    pub subject_id: Uuid,
    /// FK to `app_user.id` — the assigned person.
    pub user_id: Uuid,
    /// The working relationship / tenant role key (`property_manager`, `landlord`,
    /// `maintenance`, `leasing_agent`, `back_office`).
    pub relationship: String,
    /// The [`crate::role`] granted for this assignment's scope, if any.
    pub role_id: Option<Uuid>,
    /// The lead contact for this relationship on this subject.
    pub is_primary: bool,
    pub title: Option<String>,
    pub notes: Option<String>,
    /// The actor who created the assignment.
    pub assigned_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
