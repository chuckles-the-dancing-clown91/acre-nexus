//! `PATCH /listings/<id>` — edit a listing: pricing, copy, availability,
//! status, and public visibility (unpublish instead of delete — history and
//! applications keep their reference).

use super::dto::{ConsoleListingResp, UpdateListingReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /listings/<id>` — update fields, status, or visibility.
#[rocket_okapi::openapi(tag = "Listings")]
#[patch("/listings/<id>", data = "<body>")]
pub async fn update(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateListingReq>,
) -> ApiResult<Json<ConsoleListingResp>> {
    user.require(Permission::ListingWrite)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let listing = Listing::find_by_id(lid)
        .filter(entity::listing::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("listing not found".into()))?;

    if let Some(s) = &b.status {
        if !super::STATUSES.contains(&s.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "invalid status '{s}' (expected one of {})",
                super::STATUSES.join(", ")
            )));
        }
    }
    if let Some(r) = b.rent_cents {
        if r <= 0 {
            return Err(ApiError::BadRequest("rent_cents must be positive".into()));
        }
    }

    let mut am: entity::listing::ActiveModel = listing.into();
    if let Some(v) = b.title.filter(|t| !t.trim().is_empty()) {
        am.title = Set(v);
    }
    if let Some(v) = b.rent_cents {
        am.rent_cents = Set(v);
    }
    if let Some(v) = b.beds {
        am.beds = Set(v);
    }
    if let Some(v) = b.baths {
        am.baths = Set(v);
    }
    if let Some(v) = b.sqft {
        am.sqft = Set(v);
    }
    if let Some(v) = b.available_on {
        am.available_on = Set(v);
    }
    if let Some(v) = b.description {
        am.description = Set(v);
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.is_public {
        am.is_public = Set(v);
    }
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LISTING_UPDATE,
        Some("listing"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status, "is_public": saved.is_public })),
    )
    .await;

    crate::webhooks_out::emit(
        &db,
        scope.tenant_id,
        "listing.updated",
        serde_json::json!({
            "listing_id": saved.id,
            "title": saved.title,
            "status": saved.status,
            "rent_cents": saved.rent_cents,
            "is_public": saved.is_public,
        }),
    )
    .await;

    Ok(Json(ConsoleListingResp::from(saved)))
}
