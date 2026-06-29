//! An **impersonation session** is how platform staff enter a tenant: a
//! time-boxed (`expires_at`), reason-logged (`reason`), revocable (`revoked_at`)
//! grant that lets a [`crate::platform_staff`] act inside one tenant. Every
//! impersonated request is tagged in the audit fairing with the platform actor,
//! the reason, and the expiry. Not tenant-scoped (`tenant_id` records which
//! tenant was entered, for the trail).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "impersonation_session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub platform_staff_id: Uuid,
    pub tenant_id: Uuid,
    pub reason: String,
    pub expires_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
