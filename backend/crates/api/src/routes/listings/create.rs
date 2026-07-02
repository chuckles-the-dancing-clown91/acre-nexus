//! `POST /properties/<id>/listings` — advertise a property: create a listing
//! whose address comes from the property and whose beds/baths/sqft default
//! from the property's enrichment detail when known.

use super::dto::{ConsoleListingResp, CreateListingReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Property, PropertyDetail};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /properties/<id>/listings` — create (and by default publish) a listing.
#[rocket_okapi::openapi(tag = "Listings")]
#[post("/properties/<id>/listings", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateListingReq>,
) -> ApiResult<Json<ConsoleListingResp>> {
    user.require(Permission::ListingWrite)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();
    if b.rent_cents <= 0 {
        return Err(ApiError::BadRequest("rent_cents must be positive".into()));
    }

    let property = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let detail = PropertyDetail::find()
        .filter(entity::property_detail::Column::PropertyId.eq(pid))
        .one(&db)
        .await?;

    let title = b
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| property.name.clone());
    let now = Utc::now();
    let saved = entity::listing::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(Some(pid)),
        title: Set(title.clone()),
        address: Set(property.address.clone()),
        city: Set(property.city.clone()),
        beds: Set(b
            .beds
            .or_else(|| detail.as_ref().and_then(|d| d.beds))
            .unwrap_or(0)),
        baths: Set(b
            .baths
            .or_else(|| {
                detail
                    .as_ref()
                    .and_then(|d| d.baths.map(|x| x.floor() as i32))
            })
            .unwrap_or(1)),
        sqft: Set(b
            .sqft
            .or_else(|| detail.as_ref().and_then(|d| d.sqft))
            .unwrap_or(0)),
        rent_cents: Set(b.rent_cents),
        status: Set("Available".into()),
        available_on: Set(b
            .available_on
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Now".into())),
        description: Set(b.description.unwrap_or_default()),
        is_public: Set(b.is_public.unwrap_or(true)),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LISTING_CREATE,
        Some("listing"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": pid, "title": title })),
    )
    .await;

    Ok(Json(ConsoleListingResp::from(saved)))
}
