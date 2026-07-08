//! A single line on a [`crate::platform_invoice`] — a snapshot of one charge
//! (the plan base fee or a metered-overage line) at the moment the invoice was
//! generated, so the bill is reproducible even if plan pricing later changes.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "platform_invoice_line")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub invoice_id: Uuid,
    pub description: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
    pub sort_order: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
