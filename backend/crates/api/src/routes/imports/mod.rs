//! Import & export (see [`crate::imports`]): upload another tool's CSV, check
//! the column mapping against a preview, commit, and undo; download the
//! workspace as CSV.

use crate::audit::actions as act;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::imports::apply::{self, Counts, Made, RowOutcome};
use crate::imports::{self as im, export, undo, FieldDef, Source};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{ImportBatch, Tenant};
use rocket::data::{Data, ToByteUnit};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

const MODULE: &str = "data";
/// Rows shown in a preview besides the problems (all of which are shown).
const PREVIEW_ROWS: usize = 200;

#[derive(Serialize, schemars::JsonSchema)]
pub struct KindDef {
    pub key: &'static str,
    pub label: &'static str,
    pub fields: Vec<FieldDef>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Catalog {
    pub sources: Vec<Source>,
    pub kinds: Vec<KindDef>,
    pub datasets: Vec<export::Dataset>,
    pub max_rows: usize,
}

fn kind_label(k: &str) -> &'static str {
    match k {
        "properties" => "Properties and units",
        "tenants" => "Tenants and leases (rent roll)",
        "owners" => "Owners",
        "vendors" => "Vendors",
        _ => "",
    }
}

fn source_label(k: &str) -> &'static str {
    im::SOURCES
        .iter()
        .find(|s| s.key == k)
        .map(|s| s.label)
        .unwrap_or("a spreadsheet")
}

/// `GET /imports/catalog` — what can be imported and exported.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[get("/imports/catalog")]
pub async fn catalog(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Catalog>> {
    if user.require(Permission::DataImport).is_err() {
        user.require(Permission::DataExport)?;
    }
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    Ok(Json(Catalog {
        sources: im::SOURCES.to_vec(),
        kinds: im::KINDS
            .iter()
            .map(|k| KindDef {
                key: k,
                label: kind_label(k),
                fields: im::fields(k).to_vec(),
            })
            .collect(),
        datasets: export::DATASETS.to_vec(),
        max_rows: im::MAX_ROWS,
    }))
}

/// `GET /import-templates/<kind>` — a blank CSV with the columns the
/// importer reads, and one example row.
#[rocket_okapi::openapi(skip)]
#[get("/import-templates/<kind>")]
pub async fn template(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
) -> ApiResult<ReportFile> {
    user.require(Permission::DataImport)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let kind = kind.trim_end_matches(".csv");
    let fields = im::fields(kind);
    if fields.is_empty() {
        return Err(ApiError::NotFound("no such import".into()));
    }
    let example: BTreeMap<&str, &str> = [
        ("property", "Maple Court"),
        ("address", "123 Maple Ct"),
        ("city", "Portland"),
        ("state", "OR"),
        ("zip", "97214"),
        ("property_type", "Multifamily"),
        ("year_built", "1998"),
        ("unit", "2"),
        ("beds", "2"),
        ("baths", "1"),
        ("sqft", "880"),
        ("market_rent", "1850.00"),
        ("tenant_name", "Jordan Rivera"),
        ("email", "jordan@example.com"),
        ("phone", "(503) 555-0142"),
        ("rent", "1850.00"),
        ("deposit", "1850.00"),
        ("lease_start", "2026-01-01"),
        ("lease_end", "2026-12-31"),
        ("balance", "0.00"),
        ("status", "Current"),
        (
            "name",
            if kind == "vendors" {
                "Rapid Rooter Plumbing"
            } else {
                "Maple Holdings LLC"
            },
        ),
        ("company", ""),
        ("contact_name", "Sam Ortiz"),
        ("website", "https://rapidrooter.example"),
        ("trades", "Plumbing; HVAC"),
        ("notes", ""),
    ]
    .into_iter()
    .collect();
    let keep: Vec<&FieldDef> = fields
        .iter()
        .filter(|f| !matches!(f.key, "first_name" | "last_name" | "bd_ba" | "move_in"))
        .collect();
    let head = keep.iter().map(|f| f.key).collect::<Vec<_>>().join(",");
    let row = keep
        .iter()
        .map(|f| {
            let v = example.get(f.key).copied().unwrap_or("");
            if v.contains([',', ';', ' ']) {
                format!("\"{v}\"")
            } else {
                v.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(ReportFile::new(
        format!("{head}\n{row}\n").into_bytes(),
        "text/csv",
        format!("vantedge-{kind}-template.csv"),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BatchDto {
    pub id: Uuid,
    pub kind: String,
    pub kind_label: String,
    pub source: String,
    pub source_label: String,
    pub filename: String,
    /// `draft` | `done` | `undone`.
    pub status: String,
    pub row_count: i32,
    pub mapping: BTreeMap<String, String>,
    pub summary: serde_json::Value,
    pub errors: serde_json::Value,
    pub created_at: String,
    pub committed_at: Option<String>,
    pub undone_at: Option<String>,
}

impl From<entity::import_batch::Model> for BatchDto {
    fn from(m: entity::import_batch::Model) -> Self {
        BatchDto {
            kind_label: kind_label(&m.kind).into(),
            source_label: source_label(&m.source).into(),
            id: m.id,
            kind: m.kind,
            source: m.source,
            filename: m.filename,
            status: m.status,
            row_count: m.row_count,
            mapping: serde_json::from_value(m.mapping).unwrap_or_default(),
            summary: m.summary,
            errors: m.errors,
            created_at: m.created_at.to_rfc3339(),
            committed_at: m.committed_at.map(|d| d.to_rfc3339()),
            undone_at: m.undone_at.map(|d| d.to_rfc3339()),
        }
    }
}

/// A draft with its preview: what committing would do, row by row.
#[derive(Serialize, schemars::JsonSchema)]
pub struct Preview {
    pub batch: BatchDto,
    pub headers: Vec<String>,
    /// The first rows as they are in the file, for the mapping screen.
    pub sample: Vec<Vec<String>>,
    /// Fields that must be mapped and aren't.
    pub missing: Vec<&'static str>,
    pub counts: Counts,
    /// Every problem, then the first rows of the rest.
    pub rows: Vec<RowOutcome>,
}

async fn tenant_ctx(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<String> {
    Ok(Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default())
}

async fn preview_of(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    b: entity::import_batch::Model,
) -> ApiResult<Preview> {
    let text = b.content.clone().unwrap_or_default();
    let table = im::parse(&text).map_err(ApiError::BadRequest)?;
    let mapping: BTreeMap<String, String> =
        serde_json::from_value(b.mapping.clone()).unwrap_or_default();
    let missing = im::missing_required(&b.kind, &mapping);
    let (counts, rows) = if missing.is_empty() {
        let ctx = apply::Ctx {
            tenant_id,
            source_label: source_label(&b.source),
            today: Utc::now().date_naive().to_string(),
        };
        let o = apply::run(db, &ctx, &b.kind, &table, &mapping, false).await?;
        let mut rows: Vec<RowOutcome> = o
            .rows
            .iter()
            .filter(|r| r.action == "error")
            .cloned()
            .collect();
        rows.extend(
            o.rows
                .iter()
                .filter(|r| r.action != "error")
                .take(PREVIEW_ROWS)
                .cloned(),
        );
        (o.counts, rows)
    } else {
        (
            Counts {
                rows: table.rows.len(),
                ..Default::default()
            },
            vec![],
        )
    };
    Ok(Preview {
        sample: table.rows.iter().take(5).cloned().collect(),
        headers: table.headers,
        missing,
        counts,
        rows,
        batch: BatchDto::from(b),
    })
}

async fn find(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::import_batch::Model> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    ImportBatch::find_by_id(id)
        .filter(entity::import_batch::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("import not found".into()))
}

/// `POST /imports?kind=&filename=` — upload a CSV (the raw file as the body).
/// Returns the draft with the guessed source and mapping, and its preview.
#[rocket_okapi::openapi(skip)]
#[post("/imports?<kind>&<filename>&<source>", data = "<body>")]
pub async fn upload(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
    filename: Option<&str>,
    source: Option<&str>,
    body: Data<'_>,
) -> ApiResult<Json<Preview>> {
    user.require(Permission::DataImport)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    if !im::KINDS.contains(&kind) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of {}",
            im::KINDS.join(", ")
        )));
    }
    let bytes = body
        .open(im::MAX_BYTES.bytes())
        .into_bytes()
        .await
        .map_err(|e| ApiError::BadRequest(format!("couldn't read the upload: {e}")))?;
    if !bytes.is_complete() {
        return Err(ApiError::BadRequest("the file is larger than 10 MB".into()));
    }
    let text = im::decode(&bytes.into_inner());
    let table = im::parse(&text).map_err(ApiError::BadRequest)?;
    if table.rows.is_empty() {
        return Err(ApiError::BadRequest(
            "the file has a header row but no data".into(),
        ));
    }
    let detected = im::detect_source(&table.headers);
    let source = source
        .filter(|s| im::SOURCES.iter().any(|x| x.key == *s))
        .unwrap_or(detected);
    let mapping = im::auto_map(kind, &table.headers);
    let b = entity::import_batch::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        kind: Set(kind.into()),
        source: Set(source.into()),
        filename: Set(filename.unwrap_or("import.csv").chars().take(200).collect()),
        status: Set("draft".into()),
        content: Set(Some(text)),
        mapping: Set(json!(mapping)),
        row_count: Set(table.rows.len() as i32),
        summary: Set(json!({})),
        created: Set(json!([])),
        errors: Set(json!([])),
        created_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
        committed_at: Set(None),
        undone_at: Set(None),
        undone_by: Set(None),
    }
    .insert(&db)
    .await?;
    Ok(Json(preview_of(&db, scope.tenant_id, b).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DraftReq {
    /// Field → column header ("" or absent to leave a field unmapped).
    pub mapping: Option<BTreeMap<String, String>>,
    pub source: Option<String>,
    pub kind: Option<String>,
}

/// `PATCH /imports/<id>` — change a draft's mapping, source or kind; returns
/// the new preview.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[patch("/imports/<id>", data = "<body>")]
pub async fn update_draft(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<DraftReq>,
) -> ApiResult<Json<Preview>> {
    user.require(Permission::DataImport)?;
    let b = find(&db, scope.tenant_id, id).await?;
    if b.status != "draft" {
        return Err(ApiError::Conflict("this import has already run".into()));
    }
    let req = body.into_inner();
    let headers = im::parse(b.content.as_deref().unwrap_or(""))
        .map_err(ApiError::BadRequest)?
        .headers;
    let mut am: entity::import_batch::ActiveModel = b.clone().into();
    let kind = match req.kind {
        Some(k) if im::KINDS.contains(&k.as_str()) => {
            if k != b.kind {
                am.mapping = Set(json!(im::auto_map(&k, &headers)));
            }
            am.kind = Set(k.clone());
            k
        }
        Some(_) => return Err(ApiError::BadRequest("unknown kind".into())),
        None => b.kind.clone(),
    };
    if let Some(s) = req.source {
        if !im::SOURCES.iter().any(|x| x.key == s) {
            return Err(ApiError::BadRequest("unknown source".into()));
        }
        am.source = Set(s);
    }
    if let Some(m) = req.mapping {
        let valid: Vec<&str> = im::fields(&kind).iter().map(|f| f.key).collect();
        let clean: BTreeMap<String, String> = m
            .into_iter()
            .filter(|(k, v)| !v.is_empty() && valid.contains(&k.as_str()))
            .collect();
        if let Some((_, h)) = clean.iter().find(|(_, h)| !headers.contains(h)) {
            return Err(ApiError::BadRequest(format!(
                "the file has no column \"{h}\""
            )));
        }
        am.mapping = Set(json!(clean));
    }
    let saved = am.update(&db).await?;
    Ok(Json(preview_of(&db, scope.tenant_id, saved).await?))
}

/// `GET /imports/<id>/preview` — a draft's preview again.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[get("/imports/<id>/preview")]
pub async fn preview(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Preview>> {
    user.require(Permission::DataImport)?;
    let b = find(&db, scope.tenant_id, id).await?;
    if b.status != "draft" {
        return Err(ApiError::Conflict("this import has already run".into()));
    }
    Ok(Json(preview_of(&db, scope.tenant_id, b).await?))
}

/// `POST /imports/<id>/commit` — run the import for real.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[post("/imports/<id>/commit")]
pub async fn commit(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<BatchDto>> {
    user.require(Permission::DataImport)?;
    let b = find(&db, scope.tenant_id, id).await?;
    if b.status != "draft" {
        return Err(ApiError::Conflict("this import has already run".into()));
    }
    let table = im::parse(b.content.as_deref().unwrap_or("")).map_err(ApiError::BadRequest)?;
    let mapping: BTreeMap<String, String> =
        serde_json::from_value(b.mapping.clone()).unwrap_or_default();
    let missing = im::missing_required(&b.kind, &mapping);
    if !missing.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "map a column to {} first",
            missing.join(", ")
        )));
    }
    let ctx = apply::Ctx {
        tenant_id: scope.tenant_id,
        source_label: source_label(&b.source),
        today: Utc::now().date_naive().to_string(),
    };
    let o = apply::run(&db, &ctx, &b.kind, &table, &mapping, true).await?;
    let errors: Vec<&RowOutcome> = o.rows.iter().filter(|r| r.action == "error").collect();
    let mut am: entity::import_batch::ActiveModel = b.clone().into();
    am.status = Set("done".into());
    am.content = Set(None);
    am.summary = Set(json!(o.counts));
    am.created = Set(json!(o.made));
    am.errors = Set(json!(errors.iter().take(1000).collect::<Vec<_>>()));
    am.committed_at = Set(Some(Utc::now().into()));
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::IMPORT_COMMIT,
        Some("import_batch"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "kind": saved.kind, "source": saved.source, "filename": saved.filename, "counts": o.counts })),
    )
    .await;
    Ok(Json(BatchDto::from(saved)))
}

/// `DELETE /imports/<id>` — throw away a draft.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[delete("/imports/<id>")]
pub async fn discard(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::DataImport)?;
    let b = find(&db, scope.tenant_id, id).await?;
    if b.status != "draft" {
        return Err(ApiError::Conflict(
            "this import has run; undo it instead".into(),
        ));
    }
    ImportBatch::delete_by_id(b.id).exec(&db).await?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /imports` — past imports, newest first (drafts included).
#[rocket_okapi::openapi(tag = "Import & Export")]
#[get("/imports")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<BatchDto>>> {
    user.require(Permission::DataImport)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let rows = ImportBatch::find()
        .filter(entity::import_batch::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::import_batch::Column::CreatedAt)
        .limit(100)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|mut b| {
                b.content = None;
                BatchDto::from(b)
            })
            .collect(),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UndoResp {
    pub batch: BatchDto,
    pub report: undo::UndoReport,
}

/// `POST /imports/<id>/undo` — remove what an import made, except records
/// that have been used or changed since.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[post("/imports/<id>/undo")]
pub async fn undo_import(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<UndoResp>> {
    user.require(Permission::DataImport)?;
    let b = find(&db, scope.tenant_id, id).await?;
    if b.status != "done" {
        return Err(ApiError::Conflict(match b.status.as_str() {
            "undone" => "this import was already undone".into(),
            _ => "this import hasn't run".to_string(),
        }));
    }
    let made: Vec<Made> = serde_json::from_value(b.created.clone()).unwrap_or_default();
    let since = b.committed_at.unwrap_or(b.created_at);
    let report = undo::undo(&db, &made, since).await?;
    // Properties whose units or leases went: recount them.
    let mut am: entity::import_batch::ActiveModel = b.clone().into();
    am.status = Set("undone".into());
    am.undone_at = Set(Some(Utc::now().into()));
    am.undone_by = Set(Some(user.user_id));
    am.summary = Set({
        let mut s = b.summary.clone();
        s["undo"] = json!({ "removed": report.removed, "kept": report.kept.len() });
        s
    });
    let saved = am.update(&db).await?;
    for m in made.iter().filter(|m| m.t == "property") {
        if report.kept.iter().any(|k| k.id == m.id) {
            crate::rentals_occupancy::sync_property_occupancy(&db, m.id).await;
        }
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::IMPORT_UNDO,
        Some("import_batch"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "removed": report.removed, "kept": report.kept.len() })),
    )
    .await;
    Ok(Json(UndoResp {
        batch: BatchDto::from(saved),
        report,
    }))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DatasetCount {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub importable: bool,
    pub rows: usize,
}

/// `GET /exports` — what can be downloaded, with row counts.
#[rocket_okapi::openapi(tag = "Import & Export")]
#[get("/exports")]
pub async fn exports(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<DatasetCount>>> {
    user.require(Permission::DataExport)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let mut out = vec![];
    for d in export::DATASETS {
        let rows = export::dataset(&db, scope.tenant_id, d.key)
            .await?
            .map(|(_, n)| n)
            .unwrap_or(0);
        out.push(DatasetCount {
            key: d.key,
            label: d.label,
            description: d.description,
            importable: d.importable,
            rows,
        });
    }
    Ok(Json(out))
}

/// `GET /exports/<key>` — one dataset as CSV, or `all` as a zip.
#[rocket_okapi::openapi(skip)]
#[get("/exports/<key>")]
pub async fn download(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    key: &str,
) -> ApiResult<ReportFile> {
    user.require(Permission::DataExport)?;
    crate::modules::require_enabled(&db, scope.tenant_id, MODULE).await?;
    let key = key.trim_end_matches(".csv").trim_end_matches(".zip");
    let name = tenant_ctx(&db, scope.tenant_id).await?;
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let date = Utc::now().format("%Y-%m-%d");
    let file = if key == "all" {
        ReportFile::new(
            export::bundle(&db, scope.tenant_id, &name).await?,
            "application/zip",
            format!("{slug}-export-{date}.zip"),
        )
    } else {
        let Some((text, _)) = export::dataset(&db, scope.tenant_id, key).await? else {
            return Err(ApiError::NotFound("no such export".into()));
        };
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend(text.into_bytes());
        ReportFile::new(bytes, "text/csv", format!("{slug}-{key}-{date}.csv"))
    };
    crate::audit::record(
        &db,
        Some(user.user_id),
        act::DATA_EXPORT,
        Some("tenant"),
        Some(scope.tenant_id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "dataset": key })),
    )
    .await;
    Ok(file)
}
