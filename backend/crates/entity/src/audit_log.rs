//! An **audit log** entry records a security-relevant action — who did what, to
//! which target, in which workspace, and when. Sensitive operations (revealing
//! PII, creating/editing roles, creating users) write here so the Acre dashboard
//! can show an access trail and ship it to an external audit sink.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The user who performed the action (`NULL` for system actions).
    pub actor_user_id: Option<Uuid>,
    /// Dotted action key, e.g. `pii.reveal`, `role.update`, `user.create`.
    pub action: String,
    /// What was acted on, e.g. `user`, `role`.
    pub target_type: Option<String>,
    /// Identifier of the target (stringified id).
    pub target_id: Option<String>,
    /// Workspace context, if any.
    pub tenant_id: Option<Uuid>,
    /// Free-form structured detail.
    pub metadata: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
