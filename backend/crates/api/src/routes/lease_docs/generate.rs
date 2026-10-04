//! `POST /leases/<id>/document/generate` — render a lease agreement from the
//! tenant's legal templates + the concrete lease, its charges, and vehicles.
//! The rendering core is shared with application→lease conversion, which
//! auto-generates the first draft.

use super::dto::LeaseDocDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::leasedoc;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{
    Asset, Lease, LeaseCharge, Property, ResidentProfile, Theme, Unit, User, Vehicle,
};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

/// Render + persist a new draft lease document for `lease` — the shared core
/// behind the explicit generate endpoint and conversion's auto-generation.
pub(crate) async fn generate_for_lease(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
    actor: Option<Uuid>,
) -> ApiResult<entity::lease_document::Model> {
    let property = Property::find_by_id(lease.property_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let unit = match lease.unit_id {
        Some(uid) => Unit::find_by_id(uid).one(db).await?,
        None => None,
    };
    let charges = LeaseCharge::find()
        .filter(entity::lease_charge::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_charge::Column::LeaseId.eq(lease.id))
        .order_by_asc(entity::lease_charge::Column::CreatedAt)
        .all(db)
        .await?;
    let vehicles = Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(tenant_id))
        .filter(entity::vehicle::Column::LeaseId.eq(lease.id))
        .all(db)
        .await?;
    let templates = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|t| t.legal_templates)
        .unwrap_or_else(|| serde_json::json!({}));

    // The resident's profile, when they have an account in this company.
    let resident = match lease.tenant_email.as_deref().filter(|e| !e.is_empty()) {
        Some(email) => {
            let account = User::find()
                .filter(entity::user::Column::Email.eq(email.to_lowercase()))
                .filter(entity::user::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?;
            match account {
                Some(a) => ResidentProfile::find_by_id(a.id)
                    .one(db)
                    .await?
                    .map(|r| crate::resident::Extras::from_model(&r)),
                None => None,
            }
        }
        None => None,
    };
    let utilities =
        crate::utilities::terms(db, tenant_id, lease.property_id, lease.unit_id).await?;
    // A unit's own equipment, plus the house's when the property is one home.
    let single = crate::property_kind::unit_mode(&property.property_type)
        == crate::property_kind::UnitMode::Single;
    let mut equipment = Asset::find()
        .filter(entity::asset::Column::TenantId.eq(tenant_id))
        .filter(entity::asset::Column::PropertyId.eq(lease.property_id))
        .filter(entity::asset::Column::Status.eq("active"))
        .order_by_asc(entity::asset::Column::Name)
        .all(db)
        .await?;
    equipment.retain(|a| match a.unit_id {
        Some(u) => Some(u) == lease.unit_id,
        None => single,
    });

    let built = leasedoc::build(&leasedoc::LeaseInput {
        templates: &templates,
        lease,
        property: &property,
        unit: unit.as_ref(),
        charges: &charges,
        vehicles: &vehicles,
        resident: resident.as_ref(),
        utilities: &utilities,
        equipment: &equipment,
    });
    let body = built.body;
    let sections = serde_json::to_value(&built.sections).ok();

    let now = Utc::now();
    let saved = entity::lease_document::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        lease_id: Set(lease.id),
        title: Set(
            crate::settings::get_string(db, tenant_id, crate::settings::LEASE_DOC_TITLE).await,
        ),
        body: Set(body),
        format: Set("text".into()),
        purpose: Set("lease".into()),
        status: Set("draft".into()),
        generated_at: Set(now.into()),
        signed_at: Set(None),
        signed_by: Set(None),
        signed_hash: Set(None),
        signed_ip: Set(None),
        created_at: Set(now.into()),
        sections: Set(sections),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        actor,
        crate::audit::actions::LEASE_DOC_GENERATE,
        Some("lease_document"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "lease_id": lease.id })),
    )
    .await;
    Ok(saved)
}

/// `POST /leases/<id>/document/generate` — produce a new draft lease document.
#[rocket_okapi::openapi(tag = "Lease Documents")]
#[post("/leases/<id>/document/generate")]
pub async fn generate(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LeaseDocDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let saved = generate_for_lease(&db, scope.tenant_id, &lease, Some(user.user_id)).await?;
    Ok(Json(LeaseDocDto::from(saved)))
}
