use super::dto::PiiReveal;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::pii;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;
use uuid::Uuid;

/// `GET /admin/users/<id>/pii` — decrypt and return sensitive PII. Requires the
/// dedicated `profile:read_pii` permission and is logged as an access event.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/users/<id>/pii")]
pub async fn reveal_pii(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<PiiReveal>> {
    user.require(Permission::ProfilePiiRead)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let p = UserProfile::find_by_id(uid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("profile not found".into()))?;
    let key = &state.config.pii_key;
    let ssn = match (p.ssn_ciphertext, p.ssn_nonce) {
        (Some(ct), Some(n)) => Some(pii::decrypt(key, &ct, &n).map_err(ApiError::Internal)?),
        _ => None,
    };
    let gov = match (p.gov_id_ciphertext, p.gov_id_nonce) {
        (Some(ct), Some(n)) => Some(pii::decrypt(key, &ct, &n).map_err(ApiError::Internal)?),
        _ => None,
    };
    tracing::warn!(actor = %user.user_id, subject = %uid, "PII revealed (SSN/gov-id)");
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::PII_REVEAL,
        Some("user"),
        Some(uid.to_string()),
        None,
        Some(serde_json::json!({ "fields": ["ssn", "gov_id"] })),
    )
    .await;
    Ok(Json(PiiReveal {
        ssn,
        gov_id_number: gov,
    }))
}
