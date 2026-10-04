//! **Spanish for residents** (roadmap area 16): which language a person reads.
//!
//! A language is stored against each address we reach the person at (email,
//! lowercased; phone, E.164) in `contact_language`, so the notification job
//! picks it from the recipient alone. English is the default and the
//! fallback for any message without a Spanish version.

use chrono::Utc;
use entity::prelude::ContactLanguage;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub const LANGUAGES: &[&str] = &["en", "es"];

/// The key an address is stored under: a lowercased email, or a phone in
/// E.164. `None` for something that is neither.
pub fn contact_key(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.contains('@') {
        return Some(t.to_lowercase());
    }
    crate::texts::normalize_phone(t)
}

/// A supported language code, from what a person or form sent.
pub fn parse(raw: &str) -> Option<&'static str> {
    let l = raw.trim().to_lowercase();
    let l = l.split(['-', '_']).next().unwrap_or("");
    LANGUAGES.iter().copied().find(|x| *x == l)
}

/// Record `language` for every address given (blank ones are skipped).
pub async fn set(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    addresses: &[Option<&str>],
    language: &str,
) -> Result<usize, DbErr> {
    let mut n = 0;
    for a in addresses.iter().flatten() {
        let Some(key) = contact_key(a) else { continue };
        ContactLanguage::insert(entity::contact_language::ActiveModel {
            tenant_id: Set(tenant_id),
            contact: Set(key),
            language: Set(language.to_string()),
            updated_at: Set(Utc::now().into()),
        })
        .on_conflict(
            OnConflict::columns([
                entity::contact_language::Column::TenantId,
                entity::contact_language::Column::Contact,
            ])
            .update_columns([
                entity::contact_language::Column::Language,
                entity::contact_language::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec(db)
        .await?;
        n += 1;
    }
    Ok(n)
}

/// The language for an address; English when nothing is on file.
pub async fn for_contact(db: &impl ConnectionTrait, tenant_id: Uuid, raw: &str) -> &'static str {
    let Some(key) = contact_key(raw) else {
        return "en";
    };
    let found = ContactLanguage::find()
        .filter(entity::contact_language::Column::TenantId.eq(tenant_id))
        .filter(entity::contact_language::Column::Contact.eq(key))
        .one(db)
        .await
        .ok()
        .flatten();
    found.and_then(|r| parse(&r.language)).unwrap_or("en")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_codes() {
        assert_eq!(
            contact_key(" Ana@Example.COM ").as_deref(),
            Some("ana@example.com")
        );
        assert_eq!(
            contact_key("(503) 555-0123").as_deref(),
            Some("+15035550123")
        );
        assert_eq!(contact_key("not a thing"), None);
        assert_eq!(parse("es-MX"), Some("es"));
        assert_eq!(parse("EN"), Some("en"));
        assert_eq!(parse("fr"), None);
    }
}

// ---- routes ----------------------------------------------------------------

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, put};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct LanguageReq {
    /// `en` | `es`
    pub language: String,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct LanguageResp {
    pub language: String,
    /// The addresses it now applies to.
    pub addresses: usize,
}

fn chosen(raw: &str) -> ApiResult<&'static str> {
    parse(raw).ok_or_else(|| ApiError::BadRequest("language must be en or es".into()))
}

async fn lease_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::lease::Model> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("lease not found".into()))?;
    entity::prelude::Lease::find_by_id(id)
        .one(db)
        .await?
        .filter(|l| l.tenant_id == tenant_id)
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))
}

/// `GET /leases/<id>/language` — the language messages to this resident use.
#[rocket_okapi::openapi(tag = "Leases")]
#[get("/leases/<id>/language")]
pub async fn lease_language(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LanguageResp>> {
    user.require(Permission::LeaseRead)?;
    let l = lease_of(&db, scope.tenant_id, id).await?;
    let addr = l
        .tenant_email
        .clone()
        .or(l.tenant_phone.clone())
        .unwrap_or_default();
    Ok(Json(LanguageResp {
        language: for_contact(&db, scope.tenant_id, &addr).await.into(),
        addresses: [&l.tenant_email, &l.tenant_phone]
            .iter()
            .filter(|a| a.is_some())
            .count(),
    }))
}

/// `PUT /leases/<id>/language` — set the resident's language (their email and
/// phone on the lease).
#[rocket_okapi::openapi(tag = "Leases")]
#[put("/leases/<id>/language", data = "<body>")]
pub async fn set_lease_language(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<LanguageReq>,
) -> ApiResult<Json<LanguageResp>> {
    user.require(Permission::LeaseManage)?;
    let lang = chosen(&body.language)?;
    let l = lease_of(&db, scope.tenant_id, id).await?;
    let n = set(
        &db,
        scope.tenant_id,
        &[l.tenant_email.as_deref(), l.tenant_phone.as_deref()],
        lang,
    )
    .await?;
    if n == 0 {
        return Err(ApiError::BadRequest(
            "add the resident's email or phone to the lease first".into(),
        ));
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LANGUAGE_SET,
        Some("lease"),
        Some(l.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "language": lang })),
    )
    .await;
    Ok(Json(LanguageResp {
        language: lang.into(),
        addresses: n,
    }))
}

/// The signed-in person's addresses: their account email, plus the email and
/// phone on any lease that names them.
async fn my_addresses(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<Vec<String>> {
    let me = entity::prelude::User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let mut out = vec![me.email.clone()];
    for l in entity::prelude::Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::TenantEmail.eq(me.email.to_lowercase()))
        .all(db)
        .await?
    {
        out.extend(l.tenant_email);
        out.extend(l.tenant_phone);
    }
    Ok(out)
}

/// `GET /my/language`
#[rocket_okapi::openapi(tag = "Portal")]
#[get("/my/language")]
pub async fn my_language(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<LanguageResp>> {
    let addrs = my_addresses(&db, scope.tenant_id, user.user_id).await?;
    Ok(Json(LanguageResp {
        language: for_contact(&db, scope.tenant_id, &addrs[0]).await.into(),
        addresses: addrs.len(),
    }))
}

/// `PUT /my/language` — a resident chooses the language they get messages in.
#[rocket_okapi::openapi(tag = "Portal")]
#[put("/my/language", data = "<body>")]
pub async fn set_my_language(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<LanguageReq>,
) -> ApiResult<Json<LanguageResp>> {
    let lang = chosen(&body.language)?;
    let addrs = my_addresses(&db, scope.tenant_id, user.user_id).await?;
    let refs: Vec<Option<&str>> = addrs.iter().map(|a| Some(a.as_str())).collect();
    let n = set(&db, scope.tenant_id, &refs, lang).await?;
    Ok(Json(LanguageResp {
        language: lang.into(),
        addresses: n,
    }))
}
