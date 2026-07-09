use super::dto::{AddCommitmentReq, CommitmentDto, CommitmentListResp};
use super::{load_entity, owner_names};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{InvestorCommitment, Owner};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

const MODULE_KEY: &str = "syndication";

/// `GET /entities/<entity_id>/commitments` — the investor commitment stack.
#[rocket_okapi::openapi(tag = "Syndication")]
#[get("/entities/<entity_id>/commitments")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
) -> ApiResult<Json<CommitmentListResp>> {
    user.require(Permission::InvestorRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let llc = load_entity(&db, scope.tenant_id, entity_id).await?;

    let rows = InvestorCommitment::find()
        .filter(entity::investor_commitment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::investor_commitment::Column::EntityId.eq(llc.id))
        .order_by_asc(entity::investor_commitment::Column::CreatedAt)
        .all(&db)
        .await?;
    let names = owner_names(&db, scope.tenant_id).await?;

    let total_committed_cents = rows.iter().map(|r| r.committed_cents).sum();
    let total_contributed_cents = rows.iter().map(|r| r.contributed_cents).sum();
    let commitments = rows
        .iter()
        .map(|r| CommitmentDto::build(r, names.get(&r.owner_id).cloned().unwrap_or_default()))
        .collect();

    Ok(Json(CommitmentListResp {
        entity_id: llc.id,
        commitments,
        total_committed_cents,
        total_contributed_cents,
    }))
}

/// `POST /entities/<entity_id>/commitments` — add an investor commitment.
#[rocket_okapi::openapi(tag = "Syndication")]
#[post("/entities/<entity_id>/commitments", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
    body: Json<AddCommitmentReq>,
) -> ApiResult<Json<CommitmentDto>> {
    user.require(Permission::InvestorManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let llc = load_entity(&db, scope.tenant_id, entity_id).await?;
    let b = body.into_inner();

    if b.committed_cents <= 0 {
        return Err(ApiError::BadRequest(
            "committed_cents must be positive".into(),
        ));
    }
    let role = match b.role.as_deref().unwrap_or("investor") {
        r @ ("investor" | "manager" | "member") => r.to_string(),
        other => return Err(ApiError::BadRequest(format!("invalid role: {other}"))),
    };

    // Resolve the owner: an existing one in this tenant, or create from a name.
    let owner_id = match b.owner_id {
        Some(oid) => {
            Owner::find_by_id(oid)
                .one(&db)
                .await?
                .filter(|o| o.tenant_id == scope.tenant_id)
                .ok_or_else(|| ApiError::NotFound("owner not found".into()))?
                .id
        }
        None => {
            let name = b.owner_name.unwrap_or_default().trim().to_string();
            if name.is_empty() {
                return Err(ApiError::BadRequest(
                    "owner_id or owner_name is required".into(),
                ));
            }
            entity::owner::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                kind: Set(b.owner_kind.unwrap_or_else(|| "individual".into())),
                name: Set(name),
                email: Set(None),
                phone: Set(None),
                notes: Set(None),
                created_at: Set(Utc::now().into()),
            }
            .insert(&db)
            .await?
            .id
        }
    };

    let m = entity::investor_commitment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(llc.id),
        owner_id: Set(owner_id),
        role: Set(role),
        committed_cents: Set(b.committed_cents),
        contributed_cents: Set(0),
        returned_cents: Set(0),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::COMMITMENT_CREATE,
        Some("investor_commitment"),
        Some(m.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "entity_id": llc.id, "committed_cents": m.committed_cents })),
    )
    .await;

    let names = owner_names(&db, scope.tenant_id).await?;
    Ok(Json(CommitmentDto::build(
        &m,
        names.get(&owner_id).cloned().unwrap_or_default(),
    )))
}
