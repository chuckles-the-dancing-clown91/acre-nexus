//! **Rehab / construction management** routes (roadmap Phase 7, issue #40),
//! contributed by the `rehab` module. A [`rehab_project`](entity::rehab_project)
//! tracks a renovation budget on a property; [`rehab_draw`](entity::rehab_draw)s
//! release money against it (with progress photos via the document service);
//! [`rehab_change_order`](entity::rehab_change_order)s adjust the budget; and a
//! [`rehab_lien_waiver`](entity::rehab_lien_waiver) is generated per draw.

pub mod change_orders;
pub mod draws;
pub mod dto;
pub mod lien_waivers;
pub mod lines;
pub mod projects;

use crate::error::{ApiError, ApiResult};
use crate::storage::{sha256_hex, ObjectStore};
use chrono::Utc;
use entity::prelude::{RehabDraw, RehabProject};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// The four statutory lien-waiver types.
pub const WAIVER_TYPES: &[&str] = &[
    "conditional_progress",
    "unconditional_progress",
    "conditional_final",
    "unconditional_final",
];

/// Human label for a waiver type.
pub fn waiver_type_label(t: &str) -> &str {
    match t {
        "conditional_progress" => "Conditional waiver — progress payment",
        "unconditional_progress" => "Unconditional waiver — progress payment",
        "conditional_final" => "Conditional waiver — final payment",
        "unconditional_final" => "Unconditional waiver — final payment",
        _ => t,
    }
}

/// Load a rehab project scoped to the tenant.
pub async fn load_project(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::rehab_project::Model> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid project id".into()))?;
    RehabProject::find_by_id(pid)
        .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))
}

/// Load a rehab draw scoped to the tenant.
pub async fn load_draw(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::rehab_draw::Model> {
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid draw id".into()))?;
    RehabDraw::find_by_id(did)
        .filter(entity::rehab_draw::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("rehab draw not found".into()))
}

/// Render a lien-waiver document body (plain text; the same hand-rolled text→PDF
/// writer the e-sign envelopes use turns it into a filed PDF).
pub fn waiver_body(
    waiver_type: &str,
    contractor: &str,
    property_address: &str,
    amount_label: &str,
    through_date: Option<&str>,
    today: &str,
) -> String {
    let conditional = waiver_type.starts_with("conditional");
    let is_final = waiver_type.ends_with("final");
    let effect = if conditional {
        "This waiver is CONDITIONAL upon actual receipt of the payment described below. \
         Until payment clears, this document does not waive any lien rights."
    } else {
        "This is an UNCONDITIONAL waiver. The claimant acknowledges receipt of the \
         payment described below and releases the corresponding lien rights."
    };
    let scope = if is_final {
        "FINAL payment: this covers all work performed and materials supplied on the project."
    } else {
        "PROGRESS payment: this covers work performed and materials supplied through the date below."
    };
    let through = through_date
        .map(|d| format!("Through date: {d}"))
        .unwrap_or_default();

    format!(
        "LIEN WAIVER AND RELEASE\n\
         {label}\n\n\
         Property: {property_address}\n\
         Claimant / Contractor: {contractor}\n\
         Payment amount: {amount_label}\n\
         {through}\n\
         Date: {today}\n\n\
         {scope}\n\n\
         {effect}\n\n\
         Signature: ______________________________\n\
         Printed name: {contractor}\n",
        label = waiver_type_label(waiver_type),
    )
}

/// Generate + store a lien-waiver PDF in the document service (owner = the draw)
/// and return the new document id.
#[allow(clippy::too_many_arguments)]
pub async fn store_waiver_pdf(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    draw_id: Uuid,
    filename: &str,
    body: &str,
) -> ApiResult<Uuid> {
    let bytes = crate::pdf::text_to_pdf(body);
    let id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{id}");
    ObjectStore::from_env()?
        .put_bytes(&storage_key, &bytes)
        .await?;
    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set("rehab_draw".into()),
        owner_id: Set(draw_id),
        filename: Set(filename.to_string()),
        category: Set(Some("waiver".into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(sha256_hex(&bytes))),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(storage_key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}
