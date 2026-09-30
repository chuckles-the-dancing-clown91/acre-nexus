//! Expenses and mileage — receipts, trips, reimbursement, and what's billable
//! to the owner. `expense:read` / `expense:manage` for everyone's; anyone on
//! the team logs and fixes their own under `/me/expenses`.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::team::{my_profile, parse_id};
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use crate::workforce::{self, parse_date, Rules, EXPENSE_CATEGORIES};
use chrono::Utc;
use entity::prelude::{Document, Expense, MaintenanceTicket, Property, RehabProject, User};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ExpenseDto {
    pub id: Uuid,
    pub incurred_on: String,
    pub category: String,
    pub vendor: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    /// Mileage: miles (to two decimals) and the rate used, dollars per mile.
    pub miles: Option<f64>,
    pub mileage_rate: Option<f64>,
    pub tax_deductible: bool,
    /// `company` | `personal` | `none`
    pub vehicle: String,
    pub reimbursable: bool,
    pub reimbursed: bool,
    pub billable_to_owner: bool,
    pub billed: bool,
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub maintenance_ticket_id: Option<Uuid>,
    pub work_order_title: Option<String>,
    pub rehab_project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub property_id: Option<Uuid>,
    pub property_name: Option<String>,
    pub asset_id: Option<Uuid>,
    pub details: serde_json::Value,
    pub receipts: i64,
    pub created_at: String,
}

async fn dtos(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    rows: Vec<entity::expense::Model>,
) -> ApiResult<Vec<ExpenseDto>> {
    fn ids(v: impl Iterator<Item = Uuid>) -> Vec<Uuid> {
        let mut v: Vec<Uuid> = v.collect();
        v.sort();
        v.dedup();
        v
    }
    let users: HashMap<Uuid, String> = User::find()
        .filter(entity::user::Column::Id.is_in(ids(rows.iter().filter_map(|r| r.user_id))))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let tickets: HashMap<Uuid, String> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(
            entity::maintenance_ticket::Column::Id
                .is_in(ids(rows.iter().filter_map(|r| r.maintenance_ticket_id))),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    let projects: HashMap<Uuid, String> = RehabProject::find()
        .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
        .filter(
            entity::rehab_project::Column::Id
                .is_in(ids(rows.iter().filter_map(|r| r.rehab_project_id))),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let props: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::Id.is_in(ids(rows.iter().filter_map(|r| r.property_id))))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let mut receipts: HashMap<Uuid, i64> = HashMap::new();
    for d in Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("expense"))
        .filter(
            entity::document::Column::OwnerId.is_in(rows.iter().map(|r| r.id).collect::<Vec<_>>()),
        )
        .filter(entity::document::Column::Status.eq("stored"))
        .all(db)
        .await?
    {
        *receipts.entry(d.owner_id).or_default() += 1;
    }
    let billed = billed_live(db, tenant_id, &rows).await?;
    Ok(rows
        .into_iter()
        .map(|r| ExpenseDto {
            id: r.id,
            incurred_on: r.incurred_on,
            category: r.category,
            vendor: r.vendor,
            description: r.description,
            amount_cents: r.amount_cents,
            miles: r.miles_hundredths.map(|h| h as f64 / 100.0),
            mileage_rate: r.mileage_rate_mills.map(|m| m as f64 / 1000.0),
            tax_deductible: r.tax_deductible,
            vehicle: r.vehicle,
            reimbursable: r.reimbursable,
            reimbursed: r.reimbursed_at.is_some(),
            billable_to_owner: r.billable_to_owner,
            billed: r.billed_bill_id.is_some_and(|b| billed.contains(&b)),
            user_id: r.user_id,
            user_name: r.user_id.and_then(|u| users.get(&u).cloned()),
            maintenance_ticket_id: r.maintenance_ticket_id,
            work_order_title: r
                .maintenance_ticket_id
                .and_then(|t| tickets.get(&t).cloned()),
            rehab_project_id: r.rehab_project_id,
            project_name: r.rehab_project_id.and_then(|p| projects.get(&p).cloned()),
            property_id: r.property_id,
            property_name: r.property_id.and_then(|p| props.get(&p).cloned()),
            asset_id: r.asset_id,
            details: r.details,
            receipts: receipts.get(&r.id).copied().unwrap_or(0),
            created_at: r.created_at.to_rfc3339(),
        })
        .collect())
}

async fn billed_live(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    rows: &[entity::expense::Model],
) -> ApiResult<std::collections::HashSet<Uuid>> {
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.billed_bill_id).collect();
    if ids.is_empty() {
        return Ok(Default::default());
    }
    Ok(entity::prelude::VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_bill::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .filter(crate::workforce::costing::bill_counts)
        .map(|b| b.id)
        .collect())
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct ExpenseReq {
    pub incurred_on: String,
    pub category: String,
    pub vendor: Option<String>,
    #[serde(default)]
    pub description: String,
    /// Required except for mileage (which is priced from the miles).
    pub amount_cents: Option<i64>,
    /// Mileage: miles driven, or odometer readings in `details`.
    pub miles: Option<f64>,
    pub tax_deductible: Option<bool>,
    /// `company` | `personal` | `none`
    pub vehicle: Option<String>,
    pub reimbursable: Option<bool>,
    pub billable_to_owner: Option<bool>,
    pub maintenance_ticket_id: Option<Uuid>,
    pub rehab_project_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    /// `odometer_start`, `odometer_end`, `round_trip`, `from`, `to`, `purpose`…
    pub details: Option<serde_json::Value>,
    /// Office only: whose expense this is.
    pub user_id: Option<Uuid>,
}

/// Miles from the request: explicit, else the odometer difference (doubled
/// for a round trip), in hundredths.
pub fn miles_hundredths(miles: Option<f64>, details: &serde_json::Value) -> Option<i64> {
    let from_odo = || {
        let a = details.get("odometer_start")?.as_f64()?;
        let b = details.get("odometer_end")?.as_f64()?;
        (b > a).then_some(b - a)
    };
    let mut m = miles.or_else(from_odo)?;
    if details.get("round_trip").and_then(|v| v.as_bool()) == Some(true) && miles.is_none() {
        m *= 2.0;
    }
    (m > 0.0 && m < 10_000.0).then(|| (m * 100.0).round() as i64)
}

/// Validate and write an expense (new when `existing` is `None`).
async fn save(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Uuid,
    owner: Option<Uuid>,
    req: ExpenseReq,
    existing: Option<entity::expense::Model>,
) -> ApiResult<entity::expense::Model> {
    let rules = Rules::load(db, tenant_id).await;
    let day = parse_date(&req.incurred_on, "incurred_on")?;
    let category = req.category.trim().to_lowercase();
    if !EXPENSE_CATEGORIES.contains(&category.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "category must be one of {}",
            EXPENSE_CATEGORIES.join(", ")
        )));
    }
    let details = req.details.clone().unwrap_or_else(|| serde_json::json!({}));
    if !details.is_object() {
        return Err(ApiError::BadRequest("details must be an object".into()));
    }
    // Work it's for: a work order / project fills in the property.
    let mut property_id = req.property_id;
    if let Some(t) = req.maintenance_ticket_id {
        let tk = MaintenanceTicket::find_by_id(t)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
        property_id = Some(tk.property_id);
    }
    if let Some(p) = req.rehab_project_id {
        let pr = RehabProject::find_by_id(p)
            .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
        property_id = Some(pr.property_id);
    }
    if let Some(p) = property_id {
        Property::find_by_id(p)
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    }
    let prof = match owner {
        Some(u) => workforce::profile(db, tenant_id, u).await?,
        None => None,
    };
    let vehicle = match req.vehicle.as_deref() {
        Some(v @ ("company" | "personal" | "none")) => v.to_string(),
        Some(_) => {
            return Err(ApiError::BadRequest(
                "vehicle must be company, personal or none".into(),
            ))
        }
        None if category == "mileage" => prof
            .as_ref()
            .map(|p| p.default_vehicle.clone())
            .unwrap_or_else(|| "company".into()),
        None => "none".into(),
    };
    let (miles, rate, amount) = if category == "mileage" {
        let h = miles_hundredths(req.miles, &details).ok_or_else(|| {
            ApiError::BadRequest("say how many miles (or both odometer readings)".into())
        })?;
        let rate = rules.mileage_rate_mills;
        (
            Some(h),
            Some(rate),
            crate::workforce::overtime::div_round(h * rate, 1_000),
        )
    } else {
        let a = req
            .amount_cents
            .ok_or_else(|| ApiError::BadRequest("amount_cents is required".into()))?;
        if a <= 0 {
            return Err(ApiError::BadRequest(
                "the amount must be more than zero".into(),
            ));
        }
        (None, None, a)
    };
    // Own-vehicle miles are paid back when the person's profile says so.
    let reimbursable = req.reimbursable.unwrap_or(
        category == "mileage"
            && vehicle == "personal"
            && prof.as_ref().map(|p| p.mileage_reimbursed).unwrap_or(false),
    );
    let now = Utc::now();
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    match existing {
        Some(x) => {
            let mut am: entity::expense::ActiveModel = x.into();
            am.incurred_on = Set(day.to_string());
            am.category = Set(category);
            am.vendor = Set(clean(req.vendor));
            am.description = Set(req.description.trim().to_string());
            am.amount_cents = Set(amount);
            am.miles_hundredths = Set(miles);
            am.mileage_rate_mills = Set(rate);
            am.tax_deductible = Set(req.tax_deductible.unwrap_or(true));
            am.vehicle = Set(vehicle);
            am.reimbursable = Set(reimbursable);
            am.billable_to_owner = Set(req.billable_to_owner.unwrap_or(false));
            am.maintenance_ticket_id = Set(req.maintenance_ticket_id);
            am.rehab_project_id = Set(req.rehab_project_id);
            am.property_id = Set(property_id);
            am.asset_id = Set(req.asset_id);
            am.details = Set(details);
            am.updated_at = Set(now.into());
            Ok(am.update(db).await?)
        }
        None => Ok(entity::expense::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            incurred_on: Set(day.to_string()),
            category: Set(category),
            vendor: Set(clean(req.vendor)),
            description: Set(req.description.trim().to_string()),
            amount_cents: Set(amount),
            miles_hundredths: Set(miles),
            mileage_rate_mills: Set(rate),
            tax_deductible: Set(req.tax_deductible.unwrap_or(true)),
            vehicle: Set(vehicle),
            reimbursable: Set(reimbursable),
            reimbursed_at: Set(None),
            billable_to_owner: Set(req.billable_to_owner.unwrap_or(false)),
            billed_bill_id: Set(None),
            user_id: Set(owner),
            maintenance_ticket_id: Set(req.maintenance_ticket_id),
            rehab_project_id: Set(req.rehab_project_id),
            property_id: Set(property_id),
            asset_id: Set(req.asset_id),
            details: Set(details),
            recorded_by: Set(Some(actor)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?),
    }
}

async fn find(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::expense::Model> {
    Expense::find_by_id(parse_id(id, "expense")?)
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("expense not found".into()))
}

async fn locked(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    x: &entity::expense::Model,
) -> ApiResult<Option<&'static str>> {
    if !billed_live(db, tenant_id, std::slice::from_ref(x))
        .await?
        .is_empty()
    {
        return Ok(Some(
            "this expense is on an owner bill — void that bill first",
        ));
    }
    Ok(None)
}

async fn audit(db: &impl ConnectionTrait, actor: Uuid, action: &str, x: &entity::expense::Model) {
    crate::audit::record(
        db,
        Some(actor),
        action,
        Some("expense"),
        Some(x.id.to_string()),
        Some(x.tenant_id),
        Some(serde_json::json!({ "category": x.category, "amount_cents": x.amount_cents })),
    )
    .await;
}

fn in_window(
    q: sea_orm::Select<Expense>,
    from: Option<&str>,
    to: Option<&str>,
) -> ApiResult<sea_orm::Select<Expense>> {
    let mut q = q;
    if let Some(f) = from.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::IncurredOn.gte(parse_date(f, "from")?.to_string()));
    }
    if let Some(t) = to.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::IncurredOn.lte(parse_date(t, "to")?.to_string()));
    }
    Ok(q)
}

/// `GET /expenses?from&to&category&user_id&work_order_id&project_id&property_id`.
#[rocket_okapi::openapi(tag = "Expenses")]
#[allow(clippy::too_many_arguments)]
#[get("/expenses?<from>&<to>&<category>&<user_id>&<work_order_id>&<project_id>&<property_id>")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    category: Option<String>,
    user_id: Option<String>,
    work_order_id: Option<String>,
    project_id: Option<String>,
    property_id: Option<String>,
) -> ApiResult<Json<Vec<ExpenseDto>>> {
    user.require(Permission::ExpenseRead)?;
    let mut q = in_window(
        Expense::find().filter(entity::expense::Column::TenantId.eq(scope.tenant_id)),
        from.as_deref(),
        to.as_deref(),
    )?;
    if let Some(c) = category.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::Category.eq(c));
    }
    if let Some(u) = user_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::UserId.eq(parse_id(&u, "user")?));
    }
    if let Some(t) = work_order_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::MaintenanceTicketId.eq(parse_id(&t, "work order")?));
    }
    if let Some(p) = project_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::RehabProjectId.eq(parse_id(&p, "project")?));
    }
    if let Some(p) = property_id.filter(|s| !s.is_empty()) {
        q = q.filter(entity::expense::Column::PropertyId.eq(parse_id(&p, "property")?));
    }
    let rows = q
        .order_by_desc(entity::expense::Column::IncurredOn)
        .order_by_desc(entity::expense::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(dtos(&db, scope.tenant_id, rows).await?))
}

/// `POST /expenses` — record an expense for anyone (or the business).
#[rocket_okapi::openapi(tag = "Expenses")]
#[post("/expenses", data = "<body>")]
pub async fn create(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ExpenseReq>,
) -> ApiResult<Json<ExpenseDto>> {
    user.require(Permission::ExpenseManage)?;
    let b = body.into_inner();
    let owner = b.user_id;
    let x = save(&db, scope.tenant_id, user.user_id, owner, b, None).await?;
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_CREATE, &x).await;
    Ok(Json(dtos(&db, scope.tenant_id, vec![x]).await?.remove(0)))
}

/// `PATCH /expenses/<id>`.
#[rocket_okapi::openapi(tag = "Expenses")]
#[patch("/expenses/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ExpenseReq>,
) -> ApiResult<Json<ExpenseDto>> {
    user.require(Permission::ExpenseManage)?;
    let x = find(&db, scope.tenant_id, id).await?;
    if let Some(why) = locked(&db, scope.tenant_id, &x).await? {
        return Err(ApiError::Conflict(why.into()));
    }
    let owner = x.user_id;
    let x = save(
        &db,
        scope.tenant_id,
        user.user_id,
        owner,
        body.into_inner(),
        Some(x),
    )
    .await?;
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_UPDATE, &x).await;
    Ok(Json(dtos(&db, scope.tenant_id, vec![x]).await?.remove(0)))
}

/// `DELETE /expenses/<id>`.
#[rocket_okapi::openapi(tag = "Expenses")]
#[delete("/expenses/<id>")]
pub async fn remove(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::ExpenseManage)?;
    let x = find(&db, scope.tenant_id, id).await?;
    if let Some(why) = locked(&db, scope.tenant_id, &x).await? {
        return Err(ApiError::Conflict(why.into()));
    }
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_DELETE, &x).await;
    Expense::delete_by_id(x.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReimburseReq {
    pub ids: Vec<Uuid>,
}

/// `POST /expenses/reimburse` — mark reimbursable expenses paid back.
#[rocket_okapi::openapi(tag = "Expenses")]
#[post("/expenses/reimburse", data = "<body>")]
pub async fn reimburse(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ReimburseReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::ExpenseManage)?;
    let rows = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::expense::Column::Id.is_in(body.ids.clone()))
        .filter(entity::expense::Column::Reimbursable.eq(true))
        .filter(entity::expense::Column::ReimbursedAt.is_null())
        .all(&db)
        .await?;
    let mut total = 0;
    let n = rows.len();
    for x in rows {
        total += x.amount_cents;
        let mut am: entity::expense::ActiveModel = x.into();
        am.reimbursed_at = Set(Some(Utc::now().into()));
        let x = am.update(&db).await?;
        audit(
            &db,
            user.user_id,
            crate::audit::actions::EXPENSE_REIMBURSE,
            &x,
        )
        .await;
    }
    Ok(Json(
        serde_json::json!({ "reimbursed": n, "total_cents": total }),
    ))
}

// ---- Self-service -------------------------------------------------------------

/// `GET /me/expenses?from&to` — my expenses and trips.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/expenses?<from>&<to>")]
pub async fn mine(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Vec<ExpenseDto>>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let rows = in_window(
        Expense::find()
            .filter(entity::expense::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::expense::Column::UserId.eq(user.user_id)),
        from.as_deref(),
        to.as_deref(),
    )?
    .order_by_desc(entity::expense::Column::IncurredOn)
    .all(&db)
    .await?;
    Ok(Json(dtos(&db, scope.tenant_id, rows).await?))
}

/// `POST /me/expenses` — log a receipt or a trip.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/expenses", data = "<body>")]
pub async fn add_mine(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ExpenseReq>,
) -> ApiResult<Json<ExpenseDto>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let x = save(
        &db,
        scope.tenant_id,
        user.user_id,
        Some(user.user_id),
        body.into_inner(),
        None,
    )
    .await?;
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_CREATE, &x).await;
    Ok(Json(dtos(&db, scope.tenant_id, vec![x]).await?.remove(0)))
}

async fn my_editable(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user: &AuthUser,
    id: &str,
) -> ApiResult<entity::expense::Model> {
    let x = find(db, tenant_id, id).await?;
    if x.user_id != Some(user.user_id) {
        return Err(ApiError::NotFound("expense not found".into()));
    }
    if x.reimbursed_at.is_some() {
        return Err(ApiError::Conflict(
            "this has been paid back — ask the office to change it".into(),
        ));
    }
    if let Some(why) = locked(db, tenant_id, &x).await? {
        return Err(ApiError::Conflict(why.into()));
    }
    Ok(x)
}

/// `PATCH /me/expenses/<id>`.
#[rocket_okapi::openapi(tag = "Me")]
#[patch("/me/expenses/<id>", data = "<body>")]
pub async fn edit_mine(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ExpenseReq>,
) -> ApiResult<Json<ExpenseDto>> {
    let x = my_editable(&db, scope.tenant_id, &user, id).await?;
    let x = save(
        &db,
        scope.tenant_id,
        user.user_id,
        Some(user.user_id),
        body.into_inner(),
        Some(x),
    )
    .await?;
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_UPDATE, &x).await;
    Ok(Json(dtos(&db, scope.tenant_id, vec![x]).await?.remove(0)))
}

/// `DELETE /me/expenses/<id>`.
#[rocket_okapi::openapi(tag = "Me")]
#[delete("/me/expenses/<id>")]
pub async fn delete_mine(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let x = my_editable(&db, scope.tenant_id, &user, id).await?;
    audit(&db, user.user_id, crate::audit::actions::EXPENSE_DELETE, &x).await;
    Expense::delete_by_id(x.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---- Receipts -----------------------------------------------------------------

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReceiptReq {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    /// `receipt` | `odometer_start` | `odometer_end` | `other`
    pub label: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReceiptUpload {
    pub document_id: Uuid,
    /// PUT the file's bytes here.
    pub upload_url: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ReceiptDto {
    pub document_id: Uuid,
    pub filename: String,
    pub label: Option<String>,
    pub mime_type: String,
    pub download_url: String,
    pub created_at: String,
}

/// The expense, if the caller may attach to / see its receipts: their own, or
/// anyone's with `expense:manage` (to add) / `expense:read` (to see).
async fn receipt_access(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user: &AuthUser,
    id: &str,
    write: bool,
) -> ApiResult<entity::expense::Model> {
    let x = find(db, tenant_id, id).await?;
    let own = x.user_id == Some(user.user_id);
    let perm = if write {
        Permission::ExpenseManage
    } else {
        Permission::ExpenseRead
    };
    if !own {
        user.require(perm)?;
    }
    Ok(x)
}

/// `POST /expenses/<id>/receipts` — register a photo/PDF and get an upload URL.
#[rocket_okapi::openapi(tag = "Expenses")]
#[post("/expenses/<id>/receipts", data = "<body>")]
pub async fn add_receipt(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ReceiptReq>,
) -> ApiResult<Json<ReceiptUpload>> {
    let x = receipt_access(&db, scope.tenant_id, &user, id, true).await?;
    let filename = body.filename.trim().to_string();
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest("invalid filename".into()));
    }
    let mime = body.mime_type.trim().to_lowercase();
    if !(mime.starts_with("image/") || mime == "application/pdf") {
        return Err(ApiError::BadRequest("receipts are photos or PDFs".into()));
    }
    let size = body.size_bytes.unwrap_or(0);
    if !(0..=crate::routes::documents::MAX_SIZE_BYTES).contains(&size) {
        return Err(ApiError::BadRequest(
            "that file is too big (25 MB at most)".into(),
        ));
    }
    let label = body
        .label
        .clone()
        .filter(|l| ["receipt", "odometer_start", "odometer_end", "other"].contains(&l.as_str()));
    let doc_id = Uuid::new_v4();
    let key = format!("{}/{}", scope.tenant_id, doc_id);
    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(scope.tenant_id),
        owner_type: Set("expense".into()),
        owner_id: Set(x.id),
        filename: Set(filename),
        category: Set(Some("receipt".into())),
        requires_wet_ink: Set(false),
        physical_location: Set(label),
        mime_type: Set(mime),
        size_bytes: Set(size),
        checksum: Set(None),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(key.clone()),
        status: Set("pending_upload".into()),
        retention_expires_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    let signed = ObjectStore::from_env()
        .and_then(|s| s.signed_put_url(&key, SIGNED_URL_TTL_SECS))
        .map_err(ApiError::Internal)?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_UPLOAD,
        Some("document"),
        Some(doc_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "owner_type": "expense", "owner_id": x.id })),
    )
    .await;
    Ok(Json(ReceiptUpload {
        document_id: doc_id,
        upload_url: signed.url,
    }))
}

/// `GET /expenses/<id>/receipts` — the receipts, with short-lived download links.
#[rocket_okapi::openapi(tag = "Expenses")]
#[get("/expenses/<id>/receipts")]
pub async fn receipts(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<ReceiptDto>>> {
    let x = receipt_access(&db, scope.tenant_id, &user, id, false).await?;
    let store = ObjectStore::from_env().map_err(ApiError::Internal)?;
    let docs = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("expense"))
        .filter(entity::document::Column::OwnerId.eq(x.id))
        .filter(entity::document::Column::Status.eq("stored"))
        .order_by_asc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;
    let mut out = Vec::new();
    for d in docs {
        let url = store
            .signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS)
            .map_err(ApiError::Internal)?;
        out.push(ReceiptDto {
            document_id: d.id,
            filename: d.filename,
            label: d.physical_location,
            mime_type: d.mime_type,
            download_url: url.url,
            created_at: d.created_at.to_rfc3339(),
        });
    }
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miles_from_input_or_odometer() {
        let none = serde_json::json!({});
        assert_eq!(miles_hundredths(Some(12.345), &none), Some(1235));
        let odo = serde_json::json!({ "odometer_start": 1000.0, "odometer_end": 1012.5 });
        assert_eq!(miles_hundredths(None, &odo), Some(1250));
        let rt = serde_json::json!({ "odometer_start": 1000.0, "odometer_end": 1010.0, "round_trip": true });
        assert_eq!(miles_hundredths(None, &rt), Some(2000));
        let back = serde_json::json!({ "odometer_start": 1010.0, "odometer_end": 1000.0 });
        assert_eq!(miles_hundredths(None, &back), None);
        assert_eq!(miles_hundredths(Some(0.0), &none), None);
    }
}
