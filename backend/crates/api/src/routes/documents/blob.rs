//! Local object-store **blob endpoints** — the dev/CI stand-in for S3.
//!
//! These are what the local backend's signed URLs point at: the HMAC
//! signature + expiry in the query string *is* the authorization (exactly like
//! a presigned S3 URL), so there is no auth guard here. With
//! `STORAGE_BACKEND=s3` these routes simply 404 and signed URLs point at the
//! real store instead. Skipped in OpenAPI: clients never construct these URLs
//! themselves.

use crate::state::AppState;
use crate::storage::{sha256_hex, LocalStore, ObjectStore};
use chrono::Utc;
use entity::prelude::Document;
use rocket::http::{ContentType, Status};
use rocket::response::{self, Responder};
use rocket::{get, put, Request, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::path::{Path, PathBuf};

/// A blob with its stored content type and a download filename.
pub struct BlobResponse {
    bytes: Vec<u8>,
    content_type: String,
    filename: String,
}

impl<'r> Responder<'r, 'static> for BlobResponse {
    fn respond_to(self, _req: &'r Request<'_>) -> response::Result<'static> {
        let ct = self
            .content_type
            .parse::<ContentType>()
            .unwrap_or(ContentType::Binary);
        response::Response::build()
            .status(Status::Ok)
            .header(ct)
            .raw_header(
                "Content-Disposition",
                format!(
                    "attachment; filename=\"{}\"",
                    self.filename.replace('"', "")
                ),
            )
            .sized_body(self.bytes.len(), std::io::Cursor::new(self.bytes))
            .ok()
    }
}

fn local_store() -> Result<LocalStore, Status> {
    match ObjectStore::from_env() {
        Ok(ObjectStore::Local(s)) => Ok(s),
        Ok(ObjectStore::S3(_)) => Err(Status::NotFound),
        Err(_) => Err(Status::InternalServerError),
    }
}

fn key_from_path(path: &Path) -> Result<String, Status> {
    let key = path.to_string_lossy().replace('\\', "/");
    if key.is_empty() {
        return Err(Status::NotFound);
    }
    Ok(key)
}

/// `PUT /storage/local/<key..>?exp&sig` — receive the bytes for a signed
/// upload URL. Finalizes the matching `document` row: real size, server-side
/// SHA-256 checksum, `status=stored`.
#[rocket_okapi::openapi(skip)]
#[put("/storage/local/<key..>?<exp>&<sig>", data = "<body>")]
pub async fn put_blob(
    state: &State<AppState>,
    key: PathBuf,
    exp: i64,
    sig: &str,
    body: Vec<u8>,
) -> Result<rocket::serde::json::Json<serde_json::Value>, Status> {
    let store = local_store()?;
    let key = key_from_path(&key)?;
    if !store.verify("PUT", &key, exp, sig) {
        return Err(Status::Unauthorized);
    }

    store
        .put_bytes(&key, &body)
        .map_err(|_| Status::InternalServerError)?;

    // Finalize the metadata row with what actually arrived.
    let doc = Document::find()
        .filter(entity::document::Column::StorageKey.eq(key.clone()))
        .one(&state.db)
        .await
        .map_err(|_| Status::InternalServerError)?;
    if let Some(doc) = doc {
        let mut am: entity::document::ActiveModel = doc.into();
        am.size_bytes = Set(body.len() as i64);
        am.checksum = Set(Some(sha256_hex(&body)));
        am.status = Set("stored".into());
        am.updated_at = Set(Utc::now().into());
        if let Err(e) = am.update(&state.db).await {
            tracing::error!("failed to finalize document after upload: {e}");
        }
    }

    Ok(rocket::serde::json::Json(
        serde_json::json!({ "stored": true, "bytes": body.len() }),
    ))
}

/// `GET /storage/local/<key..>?exp&sig` — serve the bytes for a signed
/// download URL.
#[rocket_okapi::openapi(skip)]
#[get("/storage/local/<key..>?<exp>&<sig>")]
pub async fn get_blob(
    state: &State<AppState>,
    key: PathBuf,
    exp: i64,
    sig: &str,
) -> Result<BlobResponse, Status> {
    let store = local_store()?;
    let key = key_from_path(&key)?;
    if !store.verify("GET", &key, exp, sig) {
        return Err(Status::Unauthorized);
    }

    let bytes = store
        .get_bytes(&key)
        .map_err(|_| Status::InternalServerError)?
        .ok_or(Status::NotFound)?;

    // Metadata (content type + original filename) lives on the document row.
    let doc = Document::find()
        .filter(entity::document::Column::StorageKey.eq(key.clone()))
        .one(&state.db)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let (content_type, filename) = match doc {
        Some(d) => (d.mime_type, d.filename),
        None => (
            "application/octet-stream".to_string(),
            key.rsplit('/').next().unwrap_or("download").to_string(),
        ),
    };

    Ok(BlobResponse {
        bytes,
        content_type,
        filename,
    })
}
