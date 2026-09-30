use super::dto::{cadence_words, ListingResp, PublicAppliance, PublicUpkeep};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use entity::prelude::{Asset, Listing, MaintenancePlan};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /public/listings/<id>` — a single public listing.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/listings/<id>")]
pub async fn listing_detail(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
) -> ApiResult<Json<ListingResp>> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let l = Listing::find_by_id(lid)
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .filter(entity::listing::Column::Status.ne("Leased"))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("listing not found".into()))?;
    let mut resp = ListingResp::from(l.clone());
    // What the advertising team gets to say: the appliances on record and the
    // upkeep the home receives on a schedule.
    if let Some(pid) = l.property_id {
        let today = chrono::Utc::now().date_naive();
        resp.appliances = Asset::find()
            .filter(entity::asset::Column::TenantId.eq(tenant.tenant_id))
            .filter(entity::asset::Column::PropertyId.eq(pid))
            .filter(entity::asset::Column::Status.eq("active"))
            .order_by_asc(entity::asset::Column::Name)
            .all(&db)
            .await?
            .into_iter()
            .map(|a| PublicAppliance {
                under_warranty: crate::routes::maintenance::dto::warranty_state(
                    a.warranty_expires.as_deref(),
                    today,
                ) == "active",
                since: a
                    .install_date
                    .as_deref()
                    .or(a.purchased_on.as_deref())
                    .map(|d| d.chars().take(4).collect()),
                kind: a.kind,
                name: a.name,
                make: a.make,
                model: a.model,
            })
            .collect();
        resp.upkeep = MaintenancePlan::find()
            .filter(entity::maintenance_plan::Column::TenantId.eq(tenant.tenant_id))
            .filter(entity::maintenance_plan::Column::PropertyId.eq(pid))
            .filter(entity::maintenance_plan::Column::Active.eq(true))
            .order_by_asc(entity::maintenance_plan::Column::Title)
            .all(&db)
            .await?
            .into_iter()
            .map(|p| PublicUpkeep {
                title: p.title,
                cadence: cadence_words(p.cadence_days),
            })
            .collect();
    }
    Ok(Json(resp))
}
