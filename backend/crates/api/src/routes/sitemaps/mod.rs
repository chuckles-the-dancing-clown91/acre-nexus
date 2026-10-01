//! **Site maps** over HTTP: lay a property out on a map (apartment buildings
//! and units, campground sites, roads, amenities), edit the drawing in bulk,
//! import and export GeoJSON with OSM-style tags, and publish it to the public
//! site. The rules are in [`crate::sitemap`].

use crate::audit::actions as act;
use crate::audit::change::{self, Ctx};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::sitemap as rules;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::{PublicTenant, TenantScope};
use chrono::Utc;
use entity::prelude::{Document, Property, SiteFeature, SiteMap, Unit};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post, put, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Serialize, Clone, schemars::JsonSchema)]
pub struct UnitInfo {
    pub unit_number: String,
    pub status: String,
    pub market_rent_label: Option<String>,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
}

#[derive(Serialize, Clone, schemars::JsonSchema)]
pub struct FeatureDto {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    #[schemars(with = "serde_json::Value")]
    pub geometry: Value,
    pub unit_id: Option<Uuid>,
    #[schemars(with = "serde_json::Value")]
    pub attrs: Value,
    pub area_m2: Option<f64>,
    pub length_m: Option<f64>,
    pub unit: Option<UnitInfo>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Stats {
    pub features: usize,
    pub units: usize,
    pub units_available: usize,
    pub sites: usize,
    pub sites_available: usize,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MapDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub property_name: String,
    pub name: String,
    pub kind: String,
    pub base_layer: String,
    pub center_lng: Option<f64>,
    pub center_lat: Option<f64>,
    pub zoom: f64,
    pub plan_document_id: Option<Uuid>,
    #[schemars(with = "Option<serde_json::Value>")]
    pub plan_corners: Option<Value>,
    pub published: bool,
    pub notes: Option<String>,
    pub updated_at: String,
    pub stats: Stats,
    pub features: Option<Vec<FeatureDto>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateMapReq {
    pub property_id: Uuid,
    pub name: String,
    pub kind: Option<String>,
    pub base_layer: Option<String>,
    pub center_lng: Option<f64>,
    pub center_lat: Option<f64>,
    pub zoom: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateMapReq {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub base_layer: Option<String>,
    pub center_lng: Option<f64>,
    pub center_lat: Option<f64>,
    pub zoom: Option<f64>,
    pub plan_document_id: Option<Uuid>,
    #[schemars(with = "Option<serde_json::Value>")]
    pub plan_corners: Option<Value>,
    pub published: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FeatureIn {
    pub id: Option<Uuid>,
    pub kind: String,
    pub name: Option<String>,
    #[schemars(with = "serde_json::Value")]
    pub geometry: Value,
    pub unit_id: Option<Uuid>,
    #[schemars(with = "Option<serde_json::Value>")]
    pub attrs: Option<Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveFeaturesReq {
    pub features: Vec<FeatureIn>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ImportReq {
    /// A GeoJSON FeatureCollection (OSM-style tags are understood).
    #[schemars(with = "serde_json::Value")]
    pub geojson: Value,
    /// Replace the drawing instead of adding to it.
    pub replace: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlanUrl {
    pub url: String,
    pub expires_at: String,
}

fn pid(raw: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn check_view(lng: Option<f64>, lat: Option<f64>, zoom: Option<f64>) -> Result<(), ApiError> {
    if lng.is_some_and(|v| !(-180.0..=180.0).contains(&v))
        || lat.is_some_and(|v| !(-90.0..=90.0).contains(&v))
    {
        return Err(ApiError::BadRequest("centre is out of range".into()));
    }
    if zoom.is_some_and(|z| !(0.0..=24.0).contains(&z)) {
        return Err(ApiError::BadRequest("zoom must be 0 to 24".into()));
    }
    Ok(())
}

async fn find_map(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<entity::site_map::Model> {
    SiteMap::find_by_id(id)
        .filter(entity::site_map::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("map not found".into()))
}

async fn features_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    map_id: Uuid,
    public: bool,
) -> ApiResult<Vec<FeatureDto>> {
    let rows = SiteFeature::find()
        .filter(entity::site_feature::Column::TenantId.eq(tenant_id))
        .filter(entity::site_feature::Column::MapId.eq(map_id))
        .order_by_asc(entity::site_feature::Column::Position)
        .all(db)
        .await?;
    let unit_ids: Vec<Uuid> = rows.iter().filter_map(|r| r.unit_id).collect();
    let units: HashMap<Uuid, entity::unit::Model> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .filter(entity::unit::Column::Id.is_in(unit_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u))
        .collect();
    Ok(rows
        .into_iter()
        .map(|r| {
            let unit = r.unit_id.and_then(|u| units.get(&u)).map(|u| UnitInfo {
                unit_number: u.unit_number.clone(),
                // The public sees only whether a unit is available.
                status: if public && u.status != "vacant" {
                    "unavailable".into()
                } else {
                    u.status.clone()
                },
                market_rent_label: u.market_rent_cents.map(usd),
                beds: u.beds,
                baths: u.baths,
            });
            FeatureDto {
                area_m2: rules::polygon_area_m2(&r.geometry),
                length_m: rules::line_length_m(&r.geometry),
                id: r.id,
                kind: r.kind,
                name: r.name,
                geometry: r.geometry,
                // Internal ids stay out of the public view.
                unit_id: if public { None } else { r.unit_id },
                attrs: r.attrs,
                unit,
            }
        })
        .collect())
}

fn stats(fs: &[FeatureDto]) -> Stats {
    let units: Vec<_> = fs.iter().filter(|f| f.kind == "unit").collect();
    let sites: Vec<_> = fs.iter().filter(|f| f.kind == "site").collect();
    Stats {
        features: fs.len(),
        units: units.len(),
        units_available: units
            .iter()
            .filter(|f| {
                f.unit
                    .as_ref()
                    .is_some_and(|u| u.status == "vacant" || u.status == "available")
            })
            .count(),
        sites: sites.len(),
        sites_available: sites
            .iter()
            .filter(|f| {
                f.attrs["site_status"]
                    .as_str()
                    .is_none_or(|s| s == "available")
            })
            .count(),
    }
}

async fn map_dto(
    db: &impl ConnectionTrait,
    m: entity::site_map::Model,
    with_features: bool,
    public: bool,
) -> ApiResult<MapDto> {
    let fs = features_of(db, m.tenant_id, m.id, public).await?;
    let property_name = Property::find_by_id(m.property_id)
        .filter(entity::property::Column::TenantId.eq(m.tenant_id))
        .one(db)
        .await?
        .map(|p| p.name)
        .unwrap_or_default();
    Ok(MapDto {
        id: m.id,
        property_id: m.property_id,
        property_name,
        name: m.name,
        kind: m.kind,
        base_layer: m.base_layer,
        center_lng: m.center_lng,
        center_lat: m.center_lat,
        zoom: m.zoom,
        plan_document_id: if public { None } else { m.plan_document_id },
        plan_corners: m.plan_corners,
        published: m.published,
        notes: m.notes,
        updated_at: m.updated_at.to_rfc3339(),
        stats: stats(&fs),
        features: with_features.then_some(fs),
    })
}

// ---------------------------------------------------------------------------
// Maps
// ---------------------------------------------------------------------------

/// `GET /site-maps?property_id` — maps, with counts.
#[rocket_okapi::openapi(tag = "Site maps")]
#[get("/site-maps?<property_id>")]
pub async fn list_maps(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
) -> ApiResult<Json<Vec<MapDto>>> {
    user.require(Permission::PropertyRead)?;
    let mut q = SiteMap::find().filter(entity::site_map::Column::TenantId.eq(scope.tenant_id));
    if let Some(p) = property_id.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::site_map::Column::PropertyId.eq(pid(&p)?));
    }
    let rows = q
        .order_by_asc(entity::site_map::Column::Name)
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for m in rows {
        out.push(map_dto(&db, m, false, false).await?);
    }
    Ok(Json(out))
}

/// `POST /site-maps` — start a map for a property.
#[rocket_okapi::openapi(tag = "Site maps")]
#[post("/site-maps", data = "<body>")]
pub async fn create_map(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateMapReq>,
) -> ApiResult<Json<MapDto>> {
    user.require(Permission::PropertyWrite)?;
    let b = body.into_inner();
    let name = b.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let kind = b.kind.unwrap_or_else(|| "apartment".into());
    if !rules::MAP_KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of {}",
            rules::MAP_KINDS.join(", ")
        )));
    }
    let layer = b.base_layer.unwrap_or_else(|| "satellite".into());
    if !rules::BASE_LAYERS.contains(&layer.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "base_layer must be one of {}",
            rules::BASE_LAYERS.join(", ")
        )));
    }
    check_view(b.center_lng, b.center_lat, b.zoom)?;
    let prop = Property::find_by_id(b.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let now = Utc::now();
    let saved = entity::site_map::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(prop.id),
        name: Set(name),
        kind: Set(kind),
        base_layer: Set(layer),
        center_lng: Set(b.center_lng),
        center_lat: Set(b.center_lat),
        zoom: Set(b.zoom.unwrap_or(17.0)),
        plan_document_id: Set(None),
        plan_corners: Set(None),
        published: Set(false),
        notes: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    change::created(
        &db,
        Ctx::new(&user, &scope),
        act::SITE_MAP_CREATE,
        "site_map",
        saved.id,
        Some(prop.id),
        &format!("Site map {}", saved.name),
    )
    .await;
    Ok(Json(map_dto(&db, saved, true, false).await?))
}

/// `GET /site-maps/<id>` — a map with every feature.
#[rocket_okapi::openapi(tag = "Site maps")]
#[get("/site-maps/<id>")]
pub async fn get_map(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<MapDto>> {
    user.require(Permission::PropertyRead)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    Ok(Json(map_dto(&db, m, true, false).await?))
}

/// `PATCH /site-maps/<id>` — rename, change the base layer or view, attach the
/// plan image, publish.
#[rocket_okapi::openapi(tag = "Site maps")]
#[patch("/site-maps/<id>", data = "<body>")]
pub async fn update_map(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateMapReq>,
) -> ApiResult<Json<MapDto>> {
    user.require(Permission::PropertyWrite)?;
    let existing = find_map(&db, scope.tenant_id, pid(id)?).await?;
    let b = body.into_inner();
    check_view(b.center_lng, b.center_lat, b.zoom)?;
    let before = existing.clone();
    let mut am: entity::site_map::ActiveModel = existing.into();
    if let Some(n) = clean(b.name) {
        am.name = Set(n);
    }
    if let Some(k) = b.kind {
        if !rules::MAP_KINDS.contains(&k.as_str()) {
            return Err(ApiError::BadRequest("invalid kind".into()));
        }
        am.kind = Set(k);
    }
    if let Some(l) = b.base_layer {
        if !rules::BASE_LAYERS.contains(&l.as_str()) {
            return Err(ApiError::BadRequest("invalid base_layer".into()));
        }
        am.base_layer = Set(l);
    }
    if b.center_lng.is_some() {
        am.center_lng = Set(b.center_lng);
    }
    if b.center_lat.is_some() {
        am.center_lat = Set(b.center_lat);
    }
    if let Some(z) = b.zoom {
        am.zoom = Set(z);
    }
    if let Some(d) = b.plan_document_id {
        Document::find_by_id(d)
            .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::document::Column::OwnerType.eq("site_map"))
            .filter(entity::document::Column::OwnerId.eq(before.id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("upload the plan to this map first".into()))?;
        am.plan_document_id = Set(Some(d));
    }
    if let Some(c) = b.plan_corners {
        rules::validate_corners(&c).map_err(ApiError::BadRequest)?;
        am.plan_corners = Set(Some(c));
    }
    if let Some(p) = b.published {
        am.published = Set(p);
    }
    if b.notes.is_some() {
        am.notes = Set(clean(b.notes));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    change::change(
        &db,
        Ctx::new(&user, &scope),
        act::SITE_MAP_UPDATE,
        "site_map",
        saved.id,
        Some(saved.property_id),
        &format!("Site map {}", saved.name),
        &before,
        &saved,
    )
    .await;
    Ok(Json(map_dto(&db, saved, true, false).await?))
}

/// `DELETE /site-maps/<id>` — remove a map and everything drawn on it.
#[rocket_okapi::openapi(tag = "Site maps")]
#[delete("/site-maps/<id>")]
pub async fn delete_map(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Value>> {
    user.require(Permission::PropertyWrite)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    SiteFeature::delete_many()
        .filter(entity::site_feature::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::site_feature::Column::MapId.eq(m.id))
        .exec(&db)
        .await?;
    SiteMap::delete_by_id(m.id).exec(&db).await?;
    change::removed(
        &db,
        Ctx::new(&user, &scope),
        act::SITE_MAP_DELETE,
        "site_map",
        m.id,
        Some(m.property_id),
        &format!("Site map {}", m.name),
        None,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Features
// ---------------------------------------------------------------------------

struct Checked {
    kind: String,
    name: Option<String>,
    geometry: Value,
    unit_id: Option<Uuid>,
    attrs: Value,
}

async fn check_features(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    map: &entity::site_map::Model,
    ins: &[FeatureIn],
) -> ApiResult<Vec<Checked>> {
    if ins.len() > rules::MAX_FEATURES {
        return Err(ApiError::BadRequest(format!(
            "a map holds at most {} features",
            rules::MAX_FEATURES
        )));
    }
    let unit_ids: Vec<Uuid> = ins.iter().filter_map(|f| f.unit_id).collect();
    let valid_units: HashSet<Uuid> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .filter(entity::unit::Column::PropertyId.eq(map.property_id))
        .filter(entity::unit::Column::Id.is_in(unit_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| u.id)
        .collect();
    let mut linked = HashSet::new();
    let mut out = Vec::new();
    for (i, f) in ins.iter().enumerate() {
        let at = |m: String| ApiError::BadRequest(format!("feature {}: {m}", i + 1));
        if !rules::FEATURE_KINDS.contains(&f.kind.as_str()) {
            return Err(at(format!(
                "kind must be one of {}",
                rules::FEATURE_KINDS.join(", ")
            )));
        }
        rules::validate_geometry(&f.kind, &f.geometry).map_err(&at)?;
        let attrs = f.attrs.clone().unwrap_or_else(|| json!({}));
        rules::validate_attrs(&f.kind, &attrs).map_err(&at)?;
        if let Some(u) = f.unit_id {
            if f.kind != "unit" {
                return Err(at("only a unit can be linked to a unit record".into()));
            }
            if !valid_units.contains(&u) {
                return Err(at("that unit is not on this property".into()));
            }
            if !linked.insert(u) {
                return Err(at("a unit can be drawn once per map".into()));
            }
        }
        out.push(Checked {
            kind: f.kind.clone(),
            name: clean(f.name.clone()),
            geometry: f.geometry.clone(),
            unit_id: f.unit_id,
            attrs,
        });
    }
    Ok(out)
}

/// Replace the map's drawing with `checked`, matching existing rows by id.
/// Returns `(added, changed, removed)`.
async fn write_features(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    map_id: Uuid,
    ids: Vec<Option<Uuid>>,
    checked: Vec<Checked>,
    keep_others: bool,
) -> ApiResult<(usize, usize, usize)> {
    let existing: HashMap<Uuid, entity::site_feature::Model> = SiteFeature::find()
        .filter(entity::site_feature::Column::TenantId.eq(tenant_id))
        .filter(entity::site_feature::Column::MapId.eq(map_id))
        .all(db)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let now = Utc::now();
    let (mut added, mut changed) = (0, 0);
    let mut seen = HashSet::new();
    for (pos, (id, c)) in ids.into_iter().zip(checked).enumerate() {
        match id.and_then(|i| existing.get(&i)) {
            Some(old) => {
                seen.insert(old.id);
                let same = old.kind == c.kind
                    && old.name == c.name
                    && old.geometry == c.geometry
                    && old.unit_id == c.unit_id
                    && old.attrs == c.attrs
                    && old.position == pos as i32;
                if !same {
                    let mut am: entity::site_feature::ActiveModel = old.clone().into();
                    am.kind = Set(c.kind);
                    am.name = Set(c.name);
                    am.geometry = Set(c.geometry);
                    am.unit_id = Set(c.unit_id);
                    am.attrs = Set(c.attrs);
                    am.position = Set(pos as i32);
                    am.updated_at = Set(now.into());
                    am.update(db).await?;
                    changed += 1;
                }
            }
            None => {
                entity::site_feature::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    map_id: Set(map_id),
                    kind: Set(c.kind),
                    name: Set(c.name),
                    geometry: Set(c.geometry),
                    unit_id: Set(c.unit_id),
                    attrs: Set(c.attrs),
                    position: Set(pos as i32),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(db)
                .await?;
                added += 1;
            }
        }
    }
    let mut removed = 0;
    if !keep_others {
        for id in existing.keys().filter(|i| !seen.contains(i)) {
            SiteFeature::delete_by_id(*id).exec(db).await?;
            removed += 1;
        }
    }
    Ok((added, changed, removed))
}

fn summarize(added: usize, changed: usize, removed: usize) -> String {
    let mut parts = vec![];
    if added > 0 {
        parts.push(format!("{added} added"));
    }
    if changed > 0 {
        parts.push(format!("{changed} changed"));
    }
    if removed > 0 {
        parts.push(format!("{removed} removed"));
    }
    parts.join(", ")
}

/// `PUT /site-maps/<id>/features` — save the whole drawing. Features sent
/// with an existing id are updated, new ones added, and any left out removed.
#[rocket_okapi::openapi(tag = "Site maps")]
#[put("/site-maps/<id>/features", data = "<body>")]
pub async fn save_features(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SaveFeaturesReq>,
) -> ApiResult<Json<MapDto>> {
    user.require(Permission::PropertyWrite)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    let b = body.into_inner();
    let checked = check_features(&db, scope.tenant_id, &m, &b.features).await?;
    let ids = b.features.iter().map(|f| f.id).collect();
    let (a, c, r) = write_features(&db, scope.tenant_id, m.id, ids, checked, false).await?;
    if a + c + r > 0 {
        let mut am: entity::site_map::ActiveModel = m.clone().into();
        am.updated_at = Set(Utc::now().into());
        am.update(&db).await?;
        change::noted(
            &db,
            Ctx::new(&user, &scope),
            act::SITE_MAP_DRAW,
            "site_map",
            m.id,
            Some(m.property_id),
            &format!("Site map {}", m.name),
            &summarize(a, c, r),
        )
        .await;
    }
    let m = find_map(&db, scope.tenant_id, m.id).await?;
    Ok(Json(map_dto(&db, m, true, false).await?))
}

/// `GET /site-maps/<id>/export.geojson` — the drawing as GeoJSON with
/// OSM-style tags beside Vantedge's own properties.
#[rocket_okapi::openapi(tag = "Site maps")]
#[get("/site-maps/<id>/export.geojson")]
pub async fn export_geojson(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Value>> {
    user.require(Permission::PropertyRead)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    let fs = features_of(&db, scope.tenant_id, m.id, false).await?;
    let features: Vec<Value> = fs
        .iter()
        .map(|f| {
            let mut props = rules::osm_tags(&f.kind, f.name.as_deref(), &f.attrs);
            props.insert("vantedge:kind".into(), json!(f.kind));
            props.insert("vantedge:attrs".into(), f.attrs.clone());
            if let Some(u) = &f.unit {
                props.insert("ref".into(), json!(u.unit_number));
            }
            json!({ "type": "Feature", "id": f.id, "properties": props, "geometry": f.geometry })
        })
        .collect();
    Ok(Json(json!({
        "type": "FeatureCollection",
        "name": m.name,
        "features": features,
    })))
}

/// `POST /site-maps/<id>/import` — add (or replace with) the features of a
/// GeoJSON file. Vantedge exports re-import exactly; OSM pitches, roads,
/// buildings, parking and water are recognised by their tags.
#[rocket_okapi::openapi(tag = "Site maps")]
#[post("/site-maps/<id>/import", data = "<body>")]
pub async fn import_geojson(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ImportReq>,
) -> ApiResult<Json<MapDto>> {
    user.require(Permission::PropertyWrite)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    let b = body.into_inner();
    let src = b
        .geojson
        .get("features")
        .and_then(|f| f.as_array())
        .ok_or_else(|| ApiError::BadRequest("expected a GeoJSON FeatureCollection".into()))?;
    let mut ins = Vec::new();
    let mut skipped = 0;
    for f in src {
        let props = &f["properties"];
        let (kind, attrs) = match props["vantedge:kind"].as_str() {
            Some(k) if rules::FEATURE_KINDS.contains(&k) => {
                (k.to_string(), props["vantedge:attrs"].clone())
            }
            _ => match rules::kind_from_tags(props) {
                Some((k, a)) => (k.to_string(), a),
                None => {
                    skipped += 1;
                    continue;
                }
            },
        };
        let geometry = f["geometry"].clone();
        // A pitch drawn as a line or a multipolygon cannot be a site here.
        if rules::validate_geometry(&kind, &geometry).is_err() {
            skipped += 1;
            continue;
        }
        let attrs = if attrs.is_object() { attrs } else { json!({}) };
        if rules::validate_attrs(&kind, &attrs).is_err() {
            skipped += 1;
            continue;
        }
        ins.push(FeatureIn {
            id: None,
            kind,
            name: props["name"]
                .as_str()
                .or_else(|| props["ref"].as_str())
                .map(str::to_string),
            geometry,
            unit_id: None,
            attrs: Some(attrs),
        });
    }
    let replace = b.replace.unwrap_or(false);
    let have = SiteFeature::find()
        .filter(entity::site_feature::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::site_feature::Column::MapId.eq(m.id))
        .all(&db)
        .await?
        .len();
    if !replace && have + ins.len() > rules::MAX_FEATURES {
        return Err(ApiError::BadRequest(format!(
            "a map holds at most {} features",
            rules::MAX_FEATURES
        )));
    }
    let checked = check_features(&db, scope.tenant_id, &m, &ins).await?;
    let ids = vec![None; checked.len()];
    let (a, _, r) = write_features(&db, scope.tenant_id, m.id, ids, checked, !replace).await?;
    change::noted(
        &db,
        Ctx::new(&user, &scope),
        act::SITE_MAP_DRAW,
        "site_map",
        m.id,
        Some(m.property_id),
        &format!("Site map {}", m.name),
        &format!(
            "imported {a} features{}{}",
            if r > 0 {
                format!(", {r} removed")
            } else {
                String::new()
            },
            if skipped > 0 {
                format!(" ({skipped} skipped)")
            } else {
                String::new()
            }
        ),
    )
    .await;
    let m = find_map(&db, scope.tenant_id, m.id).await?;
    Ok(Json(map_dto(&db, m, true, false).await?))
}

async fn plan_url(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    m: &entity::site_map::Model,
) -> ApiResult<Json<PlanUrl>> {
    let did = m
        .plan_document_id
        .ok_or_else(|| ApiError::NotFound("this map has no plan image".into()))?;
    let doc = Document::find_by_id(did)
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("plan image not found".into()))?;
    let signed = ObjectStore::from_env()?.signed_get_url(&doc.storage_key, SIGNED_URL_TTL_SECS)?;
    Ok(Json(PlanUrl {
        url: signed.url,
        expires_at: signed.expires_at.to_rfc3339(),
    }))
}

/// `GET /site-maps/<id>/plan` — a short-lived link to the plan image.
#[rocket_okapi::openapi(tag = "Site maps")]
#[get("/site-maps/<id>/plan")]
pub async fn plan(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PlanUrl>> {
    user.require(Permission::PropertyRead)?;
    let m = find_map(&db, scope.tenant_id, pid(id)?).await?;
    plan_url(&db, scope.tenant_id, &m).await
}

// ---------------------------------------------------------------------------
// Public
// ---------------------------------------------------------------------------

/// `GET /public/site-maps?property_id` — published maps for the public site.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/site-maps?<property_id>")]
pub async fn public_maps(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    property_id: Option<String>,
) -> ApiResult<Json<Vec<MapDto>>> {
    let mut q = SiteMap::find()
        .filter(entity::site_map::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::site_map::Column::Published.eq(true));
    if let Some(p) = property_id.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::site_map::Column::PropertyId.eq(pid(&p)?));
    }
    let rows = q
        .order_by_asc(entity::site_map::Column::Name)
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for m in rows {
        out.push(map_dto(&db, m, false, true).await?);
    }
    Ok(Json(out))
}

/// `GET /public/site-maps/<id>` — one published map, without internal ids and
/// with unit status reduced to available or not.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/site-maps/<id>")]
pub async fn public_map(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
) -> ApiResult<Json<MapDto>> {
    let m = find_map(&db, tenant.tenant_id, pid(id)?).await?;
    if !m.published {
        return Err(ApiError::NotFound("map not found".into()));
    }
    Ok(Json(map_dto(&db, m, true, true).await?))
}

/// `GET /public/site-maps/<id>/plan` — the plan image of a published map.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/site-maps/<id>/plan")]
pub async fn public_plan(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    id: &str,
) -> ApiResult<Json<PlanUrl>> {
    let m = find_map(&db, tenant.tenant_id, pid(id)?).await?;
    if !m.published {
        return Err(ApiError::NotFound("map not found".into()));
    }
    plan_url(&db, tenant.tenant_id, &m).await
}
