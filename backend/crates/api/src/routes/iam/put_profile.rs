use super::dto::{ProfileDto, ProfileInput};
use super::helpers::upsert_profile_inner;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::put;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;
use uuid::Uuid;

/// `PUT /admin/users/<id>/profile` — upsert profile; SSN/gov-ID encrypted at rest.
#[rocket_okapi::openapi(tag = "IAM")]
#[put("/admin/users/<id>/profile", data = "<body>")]
pub async fn put_profile(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<ProfileInput>,
) -> ApiResult<Json<ProfileDto>> {
    user.require(Permission::ProfileWrite)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let target = User::find_by_id(uid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let input = body.into_inner();
    // Record which fields were touched, never the values (SSN/gov-id are sealed
    // at rest; the rest is still PII we don't want sitting in audit metadata).
    let mut fields = Vec::new();
    macro_rules! note {
        ($field:ident) => {
            if input.$field.is_some() {
                fields.push(stringify!($field));
            }
        };
    }
    note!(legal_first_name);
    note!(legal_middle_name);
    note!(legal_last_name);
    note!(preferred_name);
    note!(date_of_birth);
    note!(phone);
    note!(address_line1);
    note!(address_line2);
    note!(city);
    note!(region);
    note!(postal_code);
    note!(country);
    note!(photo_url);
    note!(gov_id_type);
    if input.ssn.as_deref().map(|s| !s.is_empty()).unwrap_or(false) {
        fields.push("ssn");
    }
    if input
        .gov_id_number
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        fields.push("gov_id_number");
    }

    upsert_profile_inner(&db, &state.config.pii_key, uid, &input).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROFILE_WRITE,
        Some("user"),
        Some(uid.to_string()),
        target.tenant_id,
        Some(serde_json::json!({ "fields_set": fields })),
    )
    .await;

    let p = UserProfile::find_by_id(uid).one(&db).await?.unwrap();
    Ok(Json(p.into()))
}
