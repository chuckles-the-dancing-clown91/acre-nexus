use super::dto::{resolve_assumptions, UnderwriteReq, UnderwritingDto};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::underwriting::underwrite;
use rocket::serde::json::Json;
use rocket::{post, State};

/// `POST /modules/flips/deals/<id>/underwrite` — a stateless "what-if": compute
/// underwriting for the deal with any subset of assumptions overridden. Nothing
/// is persisted, so the console can recompute live as the operator drags knobs;
/// persist a scenario by `PATCH`ing the deal.
#[rocket_okapi::openapi(tag = "Flips")]
#[post("/modules/flips/deals/<id>/underwrite", data = "<body>")]
pub async fn underwrite_deal(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UnderwriteReq>,
) -> ApiResult<Json<UnderwritingDto>> {
    user.require(Permission::DealRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    let assumptions = resolve_assumptions(&deal, Some(&body.into_inner()));
    Ok(Json(UnderwritingDto::from(underwrite(&assumptions))))
}
