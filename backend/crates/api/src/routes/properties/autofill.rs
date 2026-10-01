//! **Autofill review**: the values the property record suggests for a
//! property and its unit, applied only when a person says so.

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::autofill::{proposals, Facts, Proposal};
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Property, PropertyDetail, Unit};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct AutofillResp {
    /// When the record was last fetched; none means it has not run yet.
    pub fetched_at: Option<String>,
    pub proposals: Vec<Proposal>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ApplyReq {
    pub fields: Vec<String>,
}

struct Loaded {
    property: entity::property::Model,
    detail: Option<entity::property_detail::Model>,
    units: Vec<entity::unit::Model>,
}

async fn load(db: &impl sea_orm::ConnectionTrait, tenant_id: Uuid, id: &str) -> ApiResult<Loaded> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let property = Property::find_by_id(id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let detail = PropertyDetail::find()
        .filter(entity::property_detail::Column::TenantId.eq(tenant_id))
        .filter(entity::property_detail::Column::PropertyId.eq(id))
        .one(db)
        .await?;
    let units = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .filter(entity::unit::Column::PropertyId.eq(id))
        .all(db)
        .await?;
    Ok(Loaded {
        property,
        detail,
        units,
    })
}

fn compute(l: &Loaded) -> AutofillResp {
    let Some(d) = &l.detail else {
        return AutofillResp {
            fetched_at: None,
            proposals: vec![],
        };
    };
    let unit = (l.units.len() == 1).then(|| &l.units[0]);
    let facts = Facts {
        property_type: &l.property.property_type,
        detail_type: d.property_type.as_deref(),
        detail_beds: d.beds,
        detail_baths: d.baths,
        detail_sqft: d.sqft,
        unit_beds: unit.and_then(|u| u.beds),
        unit_baths: unit.and_then(|u| u.baths),
        unit_sqft: unit.and_then(|u| u.sqft),
        single_unit: unit.is_some(),
    };
    AutofillResp {
        fetched_at: d.last_enriched_at.map(|t| t.to_rfc3339()),
        proposals: proposals(&facts, "property record"),
    }
}

/// `GET /properties/<id>/autofill` — what the property record suggests.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>/autofill")]
pub async fn get_autofill(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<AutofillResp>> {
    user.require(Permission::PropertyRead)?;
    let l = load(&db, scope.tenant_id, id).await?;
    Ok(Json(compute(&l)))
}

/// `POST /properties/<id>/autofill/apply` — apply the chosen suggestions.
#[rocket_okapi::openapi(tag = "Properties")]
#[post("/properties/<id>/autofill/apply", data = "<body>")]
pub async fn apply_autofill(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ApplyReq>,
) -> ApiResult<Json<AutofillResp>> {
    user.require(Permission::PropertyWrite)?;
    let l = load(&db, scope.tenant_id, id).await?;
    let current = compute(&l);
    let chosen: Vec<&Proposal> = current
        .proposals
        .iter()
        .filter(|p| body.fields.contains(&p.field))
        .collect();
    if chosen.is_empty() {
        return Err(ApiError::BadRequest("nothing to apply".into()));
    }
    let ctx = Ctx::new(&user, &scope);
    let now = Utc::now();

    // The proposal already carries the property-type code.
    if let Some(p) = chosen.iter().find(|p| p.field == "property_type") {
        let before = l.property.clone();
        let mut am: entity::property::ActiveModel = l.property.clone().into();
        am.property_type = Set(p.proposed.clone());
        let saved = am.update(&db).await?;
        change::change(
            &db,
            ctx,
            act::PROPERTY_AUTOFILL,
            "property",
            saved.id,
            Some(saved.id),
            &saved.name,
            &before,
            &saved,
        )
        .await;
    }

    if l.units.len() == 1 {
        let before = l.units[0].clone();
        let mut am: entity::unit::ActiveModel = l.units[0].clone().into();
        let mut touched = false;
        for p in &chosen {
            match p.field.as_str() {
                "unit_beds" => {
                    am.beds = Set(p.proposed.parse().ok());
                    touched = true;
                }
                "unit_baths" => {
                    am.baths = Set(p.proposed.parse().ok());
                    touched = true;
                }
                "unit_sqft" => {
                    am.sqft = Set(p.proposed.parse().ok());
                    touched = true;
                }
                _ => {}
            }
        }
        if touched {
            am.updated_at = Set(now.into());
            let saved = am.update(&db).await?;
            change::change(
                &db,
                ctx,
                act::PROPERTY_AUTOFILL,
                "unit",
                saved.id,
                Some(saved.property_id),
                &format!("Unit {}", saved.unit_number),
                &before,
                &saved,
            )
            .await;
        }
    }
    let l = load(&db, scope.tenant_id, id).await?;
    Ok(Json(compute(&l)))
}
