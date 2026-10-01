//! **Listing photos** (fix plan F18): upload through the documents flow
//! (`owner_type = listing`), then attach here with alt text, a caption and an
//! order. The first photo is the hero on the listing page, the share image and
//! the structured-data image. The public address redirects to a short-lived
//! signed URL, so the blob store stays private.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::public::dto::{ListingResp, PublicPhoto};
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Document, Listing, ListingPhoto};
use rocket::response::Redirect;
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Most photos on one listing.
pub const MAX_PHOTOS: usize = 30;
const MAX_ALT: usize = 200;
const MAX_CAPTION: usize = 300;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ListingPhotoDto {
    pub id: Uuid,
    pub document_id: Uuid,
    pub alt_text: String,
    pub caption: Option<String>,
    pub position: i32,
    /// The address the public site uses (works once the listing is public).
    pub public_url: String,
    /// A signed link staff can view now, valid for 15 minutes.
    pub preview_url: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddPhotoReq {
    pub document_id: Uuid,
    /// What the picture shows, for screen readers and search ("Sunny kitchen
    /// with a gas range"). Required.
    pub alt_text: String,
    pub caption: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdatePhotoReq {
    pub alt_text: Option<String>,
    pub caption: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OrderReq {
    /// Every photo id on the listing, hero first.
    pub ids: Vec<Uuid>,
}

/// The public address of a photo.
pub fn public_url(photo_id: Uuid) -> String {
    format!(
        "{}/public/listing-photos/{photo_id}",
        crate::oauth::public_api_url().trim_end_matches('/')
    )
}

fn clean_alt(raw: &str) -> ApiResult<String> {
    let alt = raw.trim();
    if alt.is_empty() {
        return Err(ApiError::BadRequest(
            "describe the photo (alt text) so everyone can read the listing".into(),
        ));
    }
    if alt.chars().count() > MAX_ALT {
        return Err(ApiError::BadRequest(format!(
            "keep alt text under {MAX_ALT} characters"
        )));
    }
    Ok(alt.to_string())
}

fn clean_caption(raw: Option<String>) -> ApiResult<Option<String>> {
    let c = raw.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if c.as_ref().is_some_and(|c| c.chars().count() > MAX_CAPTION) {
        return Err(ApiError::BadRequest(format!(
            "keep captions under {MAX_CAPTION} characters"
        )));
    }
    Ok(c)
}

async fn listing(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::listing::Model> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Listing::find_by_id(lid)
        .filter(entity::listing::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("listing not found".into()))
}

async fn photo(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::listing_photo::Model> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    ListingPhoto::find_by_id(pid)
        .filter(entity::listing_photo::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("photo not found".into()))
}

async fn photos_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    listing_id: Uuid,
) -> ApiResult<Vec<ListingPhotoDto>> {
    let rows = ListingPhoto::find()
        .filter(entity::listing_photo::Column::TenantId.eq(tenant_id))
        .filter(entity::listing_photo::Column::ListingId.eq(listing_id))
        .order_by_asc(entity::listing_photo::Column::Position)
        .order_by_asc(entity::listing_photo::Column::CreatedAt)
        .all(db)
        .await?;
    let keys: HashMap<Uuid, String> = Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::Id.is_in(rows.iter().map(|r| r.document_id)))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.id, d.storage_key))
        .collect();
    let store = ObjectStore::from_env().ok();
    Ok(rows
        .into_iter()
        .map(|r| ListingPhotoDto {
            preview_url: keys.get(&r.document_id).and_then(|k| {
                store
                    .as_ref()?
                    .signed_get_url(k, SIGNED_URL_TTL_SECS)
                    .ok()
                    .map(|s| s.url)
            }),
            public_url: public_url(r.id),
            id: r.id,
            document_id: r.document_id,
            alt_text: r.alt_text,
            caption: r.caption,
            position: r.position,
        })
        .collect())
}

/// Fill `photos` on public listings, in order.
pub async fn attach_public(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    listings: &mut [ListingResp],
) -> ApiResult<()> {
    if listings.is_empty() {
        return Ok(());
    }
    let rows = ListingPhoto::find()
        .filter(entity::listing_photo::Column::TenantId.eq(tenant_id))
        .filter(entity::listing_photo::Column::ListingId.is_in(listings.iter().map(|l| l.id)))
        .order_by_asc(entity::listing_photo::Column::Position)
        .order_by_asc(entity::listing_photo::Column::CreatedAt)
        .all(db)
        .await?;
    let mut by: HashMap<Uuid, Vec<PublicPhoto>> = HashMap::new();
    for r in rows {
        by.entry(r.listing_id).or_default().push(PublicPhoto {
            url: public_url(r.id),
            alt: r.alt_text,
            caption: r.caption,
        });
    }
    for l in listings {
        l.photos = by.remove(&l.id).unwrap_or_default();
    }
    Ok(())
}

/// `GET /listings/<id>/photos` — the listing's photos, hero first.
#[rocket_okapi::openapi(tag = "Listings")]
#[get("/listings/<id>/photos")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<ListingPhotoDto>>> {
    user.require(Permission::ListingRead)?;
    let l = listing(&db, scope.tenant_id, id).await?;
    Ok(Json(photos_of(&db, scope.tenant_id, l.id).await?))
}

/// `POST /listings/<id>/photos` — attach an uploaded image (a document filed
/// on this listing) with its alt text. It goes to the end of the list.
#[rocket_okapi::openapi(tag = "Listings")]
#[post("/listings/<id>/photos", data = "<body>")]
pub async fn add(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddPhotoReq>,
) -> ApiResult<Json<Vec<ListingPhotoDto>>> {
    user.require(Permission::ListingWrite)?;
    let l = listing(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    let alt = clean_alt(&b.alt_text)?;
    let caption = clean_caption(b.caption)?;
    let doc = Document::find_by_id(b.document_id)
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
    if doc.owner_type != "listing" || doc.owner_id != l.id {
        return Err(ApiError::BadRequest(
            "upload the photo to this listing first".into(),
        ));
    }
    if !doc.mime_type.starts_with("image/") {
        return Err(ApiError::BadRequest(
            "only images can be listing photos".into(),
        ));
    }
    let existing = ListingPhoto::find()
        .filter(entity::listing_photo::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::listing_photo::Column::ListingId.eq(l.id))
        .all(&db)
        .await?;
    if existing.iter().any(|p| p.document_id == doc.id) {
        return Err(ApiError::Conflict(
            "that photo is already on the listing".into(),
        ));
    }
    if existing.len() >= MAX_PHOTOS {
        return Err(ApiError::BadRequest(format!(
            "a listing holds up to {MAX_PHOTOS} photos"
        )));
    }
    let next = existing.iter().map(|p| p.position + 1).max().unwrap_or(0);
    let now = Utc::now();
    entity::listing_photo::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        listing_id: Set(l.id),
        document_id: Set(doc.id),
        alt_text: Set(alt),
        caption: Set(caption),
        position: Set(next),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    Ok(Json(photos_of(&db, scope.tenant_id, l.id).await?))
}

/// `PATCH /listing-photos/<id>` — change alt text or caption.
#[rocket_okapi::openapi(tag = "Listings")]
#[patch("/listing-photos/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdatePhotoReq>,
) -> ApiResult<Json<Vec<ListingPhotoDto>>> {
    user.require(Permission::ListingWrite)?;
    let p = photo(&db, scope.tenant_id, id).await?;
    let listing_id = p.listing_id;
    let b = body.into_inner();
    let mut am: entity::listing_photo::ActiveModel = p.into();
    if let Some(alt) = b.alt_text {
        am.alt_text = Set(clean_alt(&alt)?);
    }
    if b.caption.is_some() {
        am.caption = Set(clean_caption(b.caption)?);
    }
    am.updated_at = Set(Utc::now().into());
    am.update(&db).await?;
    Ok(Json(photos_of(&db, scope.tenant_id, listing_id).await?))
}

/// `PUT /listings/<id>/photos/order` — set the order; the first is the hero.
#[rocket_okapi::openapi(tag = "Listings")]
#[put("/listings/<id>/photos/order", data = "<body>")]
pub async fn reorder(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<OrderReq>,
) -> ApiResult<Json<Vec<ListingPhotoDto>>> {
    user.require(Permission::ListingWrite)?;
    let l = listing(&db, scope.tenant_id, id).await?;
    let rows = ListingPhoto::find()
        .filter(entity::listing_photo::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::listing_photo::Column::ListingId.eq(l.id))
        .all(&db)
        .await?;
    let mut want = body.ids.clone();
    want.dedup();
    let mut have: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut sorted = want.clone();
    sorted.sort();
    have.sort();
    if sorted != have {
        return Err(ApiError::BadRequest(
            "send every photo on the listing, each once".into(),
        ));
    }
    let now = Utc::now();
    for row in rows {
        let pos = want.iter().position(|w| *w == row.id).unwrap_or(0) as i32;
        if row.position != pos {
            let mut am: entity::listing_photo::ActiveModel = row.into();
            am.position = Set(pos);
            am.updated_at = Set(now.into());
            am.update(&db).await?;
        }
    }
    Ok(Json(photos_of(&db, scope.tenant_id, l.id).await?))
}

/// `DELETE /listing-photos/<id>` — take a photo off the listing (the file
/// stays in the listing's documents).
#[rocket_okapi::openapi(tag = "Listings")]
#[delete("/listing-photos/<id>")]
pub async fn remove(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<ListingPhotoDto>>> {
    user.require(Permission::ListingWrite)?;
    let p = photo(&db, scope.tenant_id, id).await?;
    ListingPhoto::delete_by_id(p.id).exec(&db).await?;
    Ok(Json(photos_of(&db, scope.tenant_id, p.listing_id).await?))
}

/// `GET /public/listing-photos/<id>` — the image of a public listing, by way
/// of a short-lived signed link. Photos of private or leased listings are not
/// served.
#[rocket_okapi::openapi(skip)]
#[get("/public/listing-photos/<id>")]
pub async fn public_photo(state: &State<AppState>, id: &str) -> ApiResult<Redirect> {
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::NotFound("photo not found".into()))?;
    let not_found = || ApiError::NotFound("photo not found".into());
    let p = ListingPhoto::find_by_id(pid)
        .one(&state.db)
        .await?
        .ok_or_else(not_found)?;
    let l = Listing::find_by_id(p.listing_id)
        .filter(entity::listing::Column::TenantId.eq(p.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(not_found)?;
    if !l.is_public || l.status == "Leased" {
        return Err(not_found());
    }
    let doc = Document::find_by_id(p.document_id)
        .filter(entity::document::Column::TenantId.eq(p.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(not_found)?;
    let signed = ObjectStore::from_env()?.signed_get_url(&doc.storage_key, SIGNED_URL_TTL_SECS)?;
    Ok(Redirect::temporary(signed.url))
}
