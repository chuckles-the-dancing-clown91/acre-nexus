use super::{invoice_table, InvoiceDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::reports::{export, ReportFile};
use crate::tenancy::TenantScope;
use entity::prelude::{PlatformInvoice, PlatformInvoiceLine};
use rocket::get;
use rocket::serde::json::Json;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// Load an invoice + its lines, scoped to the active tenant (RLS also enforces
/// this). 404 if it isn't the caller's.
async fn load(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: Uuid,
) -> ApiResult<(
    entity::platform_invoice::Model,
    Vec<entity::platform_invoice_line::Model>,
)> {
    let inv = PlatformInvoice::find_by_id(id)
        .filter(entity::platform_invoice::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("invoice".into()))?;
    let lines = PlatformInvoiceLine::find()
        .filter(entity::platform_invoice_line::Column::InvoiceId.eq(id))
        .order_by_asc(entity::platform_invoice_line::Column::SortOrder)
        .all(db)
        .await?;
    Ok((inv, lines))
}

/// `GET /billing/invoices` — this workspace's platform invoices, newest first.
#[rocket_okapi::openapi(tag = "Billing")]
#[get("/billing/invoices")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<InvoiceDto>>> {
    user.require(Permission::BillingRead)?;
    let invoices = PlatformInvoice::find()
        .filter(entity::platform_invoice::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::platform_invoice::Column::Period)
        .all(&db)
        .await?;

    let mut out = Vec::with_capacity(invoices.len());
    for inv in invoices {
        let lines = PlatformInvoiceLine::find()
            .filter(entity::platform_invoice_line::Column::InvoiceId.eq(inv.id))
            .order_by_asc(entity::platform_invoice_line::Column::SortOrder)
            .all(&db)
            .await?;
        out.push(InvoiceDto::from(inv, lines));
    }
    Ok(Json(out))
}

/// `GET /billing/invoices/<id>` — one invoice with its line items.
#[rocket_okapi::openapi(tag = "Billing")]
#[get("/billing/invoices/<id>")]
pub async fn get(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<InvoiceDto>> {
    user.require(Permission::BillingRead)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid invoice id".into()))?;
    let (inv, lines) = load(&db, scope.tenant_id, id).await?;
    Ok(Json(InvoiceDto::from(inv, lines)))
}

/// `GET /billing/invoices/<id>/export?<format>` — CSV or PDF of the invoice.
#[rocket_okapi::openapi(skip)]
#[get("/billing/invoices/<id>/export?<format>")]
pub async fn export_invoice(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::BillingRead)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid invoice id".into()))?;
    let (inv, lines) = load(&db, scope.tenant_id, id).await?;
    let dto = InvoiceDto::from(inv.clone(), lines);
    let basename = format!("invoice-{}", inv.period);
    export(
        &invoice_table(&dto),
        &basename,
        format.as_deref().unwrap_or("pdf"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::period_label;

    #[test]
    fn period_label_formats_month() {
        assert_eq!(period_label("2026-06"), "June 2026");
        assert_eq!(period_label("2025-12"), "December 2025");
        // Malformed input is returned verbatim.
        assert_eq!(period_label("nonsense"), "nonsense");
    }
}
