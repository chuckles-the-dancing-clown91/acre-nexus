//! One import from another tool (a rent roll, a property list, an owner or
//! vendor list): the file while it's a draft, how its columns map, and once
//! committed, what it made, so it can be undone.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "import_batch")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `properties` | `tenants` | `owners` | `vendors`.
    pub kind: String,
    /// `appfolio` | `buildium` | `yardi` | `rentmanager` | `doorloop` | `vantedge` | `generic`.
    pub source: String,
    pub filename: String,
    /// `draft` | `done` | `undone`.
    pub status: String,
    /// The CSV while a draft; cleared once committed.
    #[sea_orm(column_type = "Text", nullable)]
    pub content: Option<String>,
    /// Field → column header.
    pub mapping: Json,
    pub row_count: i32,
    /// Counts by outcome.
    pub summary: Json,
    /// What the commit made: `[{ "t": "property", "id": … }]`.
    pub created: Json,
    /// Row problems: `[{ "row": 4, "message": … }]`.
    pub errors: Json,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub committed_at: Option<DateTimeWithTimeZone>,
    pub undone_at: Option<DateTimeWithTimeZone>,
    pub undone_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
