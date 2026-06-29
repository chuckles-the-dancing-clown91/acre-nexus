//! LLC branding endpoints: read and upsert the one-per-LLC branding row (logo
//! reference, colours, signature block, letterhead/footer verbiage).

use super::dto::{BrandingDto, UpdateBrandingReq};
use super::helpers::{parse_uuid, require_llc};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{LlcBranding, LlcDocument};
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

async fn load(
    state: &AppState,
    tenant_id: Uuid,
    llc_id: Uuid,
) -> ApiResult<Option<entity::llc_branding::Model>> {
    Ok(LlcBranding::find()
        .filter(entity::llc_branding::Column::TenantId.eq(tenant_id))
        .filter(entity::llc_branding::Column::LlcId.eq(llc_id))
        .one(&state.property_db)
        .await?)
}

/// `GET /llcs/<id>/branding` — current branding (empty shape if not yet set).
#[rocket_okapi::openapi(tag = "LLCs")]
#[get("/llcs/<id>/branding")]
pub async fn get_branding(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<BrandingDto>> {
    user.require(Permission::LlcRead)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let dto = match load(state, scope.tenant_id, llc_id).await? {
        Some(b) => BrandingDto::from(b),
        None => BrandingDto {
            llc_id,
            logo_document_id: None,
            primary_color: None,
            accent_color: None,
            signature_name: None,
            signature_title: None,
            signature_block: None,
            letterhead: None,
            footer: None,
        },
    };
    Ok(Json(dto))
}

/// `PUT /llcs/<id>/branding` — replace the LLC's branding (upsert).
#[rocket_okapi::openapi(tag = "LLCs")]
#[put("/llcs/<id>/branding", data = "<body>")]
pub async fn put_branding(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateBrandingReq>,
) -> ApiResult<Json<BrandingDto>> {
    user.require(Permission::LlcManage)?;
    let llc_id = parse_uuid(id)?;
    require_llc(state, scope.tenant_id, llc_id).await?;
    let req = body.into_inner();

    // A referenced logo must be one of *this* LLC's own documents.
    if let Some(logo_id) = req.logo_document_id {
        let owns = LlcDocument::find_by_id(logo_id)
            .filter(entity::llc_document::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::llc_document::Column::LlcId.eq(llc_id))
            .one(&state.property_db)
            .await?
            .is_some();
        if !owns {
            return Err(ApiError::BadRequest(
                "logo_document_id must reference a document of this LLC".into(),
            ));
        }
    }

    let now = Utc::now();

    let saved = match load(state, scope.tenant_id, llc_id).await? {
        Some(existing) => {
            let mut am: entity::llc_branding::ActiveModel = existing.into();
            am.logo_document_id = Set(req.logo_document_id);
            am.primary_color = Set(req.primary_color);
            am.accent_color = Set(req.accent_color);
            am.signature_name = Set(req.signature_name);
            am.signature_title = Set(req.signature_title);
            am.signature_block = Set(req.signature_block);
            am.letterhead = Set(req.letterhead);
            am.footer = Set(req.footer);
            am.updated_at = Set(now.into());
            am.update(&state.property_db).await?
        }
        None => entity::llc_branding::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            llc_id: Set(llc_id),
            logo_document_id: Set(req.logo_document_id),
            primary_color: Set(req.primary_color),
            accent_color: Set(req.accent_color),
            signature_name: Set(req.signature_name),
            signature_title: Set(req.signature_title),
            signature_block: Set(req.signature_block),
            letterhead: Set(req.letterhead),
            footer: Set(req.footer),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&state.property_db)
        .await?,
    };

    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::LLC_BRANDING_UPDATE,
        Some("llc_branding"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "llc_id": llc_id })),
    )
    .await;
    Ok(Json(BrandingDto::from(saved)))
}
