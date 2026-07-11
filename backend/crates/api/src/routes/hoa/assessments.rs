use super::dto::{AssessmentDto, CreateAssessmentReq};
use super::{load_association, load_member, MODULE_KEY};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{HoaAssessment, HoaMember};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// `POST /hoa/associations/<association_id>/assessments` — assess dues. With a
/// `member_id`, bills that one member; without, bills **every active member**.
#[rocket_okapi::openapi(tag = "HOA")]
#[post("/hoa/associations/<association_id>/assessments", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
    body: Json<CreateAssessmentReq>,
) -> ApiResult<Json<Vec<AssessmentDto>>> {
    user.require(Permission::HoaManage)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let b = body.into_inner();

    let amount = b.amount_cents.unwrap_or(assoc.dues_cents).max(0);
    if amount <= 0 {
        return Err(ApiError::BadRequest(
            "amount_cents must be positive (or set the association's dues)".into(),
        ));
    }
    let description = b
        .description
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| match &b.period {
            Some(p) => format!("Dues — {p}"),
            None => "Dues".into(),
        });

    // Resolve the target members: one, or every active member.
    let members: Vec<entity::hoa_member::Model> = match b.member_id {
        Some(mid) => vec![load_member(&db, scope.tenant_id, assoc.id, mid).await?],
        None => {
            HoaMember::find()
                .filter(entity::hoa_member::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::hoa_member::Column::AssociationId.eq(assoc.id))
                .filter(entity::hoa_member::Column::Status.eq("active"))
                .order_by_asc(entity::hoa_member::Column::Name)
                .all(&db)
                .await?
        }
    };
    if members.is_empty() {
        return Err(ApiError::BadRequest("no active members to assess".into()));
    }

    let now = Utc::now();
    let mut created = Vec::with_capacity(members.len());
    for m in &members {
        let a = entity::hoa_assessment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            association_id: Set(assoc.id),
            member_id: Set(m.id),
            description: Set(description.clone()),
            amount_cents: Set(amount),
            period: Set(b.period.clone()),
            due_date: Set(b.due_date.clone()),
            status: Set("due".into()),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
        created.push(AssessmentDto::from(a));
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::HOA_ASSESSMENT_CREATE,
        Some("hoa_assessment"),
        Some(assoc.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "association_id": assoc.id,
            "count": created.len(),
            "amount_cents": amount,
        })),
    )
    .await;

    Ok(Json(created))
}

/// `GET /hoa/associations/<association_id>/assessments` — dues history.
#[rocket_okapi::openapi(tag = "HOA")]
#[get("/hoa/associations/<association_id>/assessments")]
pub async fn list(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    association_id: &str,
) -> ApiResult<Json<Vec<AssessmentDto>>> {
    user.require(Permission::HoaRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, MODULE_KEY).await?;
    let assoc = load_association(&db, scope.tenant_id, association_id).await?;
    let rows = HoaAssessment::find()
        .filter(entity::hoa_assessment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::hoa_assessment::Column::AssociationId.eq(assoc.id))
        .order_by_desc(entity::hoa_assessment::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(AssessmentDto::from).collect()))
}
