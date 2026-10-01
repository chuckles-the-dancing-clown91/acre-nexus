use super::{export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use crate::vendor_compliance;
use chrono::Datelike;
use entity::prelude::{
    Counterparty, Lease, LeasePayment, Llc, Property, VendorBill, VendorTaxProfile,
};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// The IRS information-return threshold — file for a recipient paid this much
/// or more in the calendar year ($600).
const THRESHOLD_CENTS: i64 = 60_000;

#[derive(Serialize, schemars::JsonSchema)]
pub struct Recipient1099 {
    pub form: String,
    pub box_label: String,
    pub recipient_id: Uuid,
    pub name: String,
    /// Taxpayer ID on file (EIN for legal entities); vendors collect theirs via W-9.
    pub tin: Option<String>,
    /// True when a vendor has no W-9 on file, so the form can't be filed yet.
    pub missing_tin: bool,
    pub address: Option<String>,
    pub amount_cents: i64,
    pub amount_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Tax1099Resp {
    pub generated_at: String,
    pub year: i32,
    pub threshold_cents: i64,
    pub threshold_label: String,
    /// 1099-NEC — nonemployee compensation paid to vendors / contractors.
    pub nec: Vec<Recipient1099>,
    /// 1099-MISC — gross rents collected on behalf of property owners.
    pub misc: Vec<Recipient1099>,
    pub nec_total_cents: i64,
    pub nec_total_label: String,
    pub misc_total_cents: i64,
    pub misc_total_label: String,
    /// NEC recipients still missing a W-9.
    pub missing_tin_count: usize,
}

/// 1099-NEC: vendors paid (settled bills) at or above the threshold in `year`.
///
/// The on-screen report shows only the last four of each TIN; the export
/// (`reveal`) carries the full number for filing.
async fn nec_recipients(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    year: i32,
    reveal: bool,
) -> ApiResult<Vec<Recipient1099>> {
    let bills = VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_bill::Column::Status.eq("paid"))
        .all(db)
        .await?;
    let mut by_vendor: HashMap<Uuid, i64> = HashMap::new();
    for b in bills {
        let in_year = b.paid_at.map(|t| t.year() == year).unwrap_or(false);
        if in_year {
            *by_vendor.entry(b.counterparty_id).or_insert(0) += b.amount_cents;
        }
    }

    let vendors: HashMap<Uuid, entity::counterparty::Model> = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();
    let w9s: HashMap<Uuid, entity::vendor_tax_profile::Model> = VendorTaxProfile::find()
        .filter(entity::vendor_tax_profile::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.counterparty_id, p))
        .collect();

    let mut out: Vec<Recipient1099> = vec![];
    for (id, cents) in by_vendor {
        if cents < THRESHOLD_CENTS {
            continue;
        }
        let v = vendors.get(&id);
        let w9 = w9s.get(&id);
        let tin = match w9 {
            Some(_) if reveal => vendor_compliance::reveal_tin(db, tenant_id, id).await,
            Some(p) => Some(vendor_compliance::mask_tin(&p.tin_type, &p.tin_last4)),
            None => None,
        };
        // The W-9's legal name is the one the IRS matches against the TIN.
        let name = w9
            .map(|p| p.legal_name.clone())
            .or_else(|| v.map(|c| c.name.clone()))
            .unwrap_or_else(|| "—".into());
        out.push(Recipient1099 {
            form: "1099-NEC".into(),
            box_label: "Box 1 — Nonemployee compensation".into(),
            recipient_id: id,
            name,
            missing_tin: w9.is_none(),
            tin,
            address: v.and_then(|c| c.address.clone()),
            amount_cents: cents,
            amount_label: usd(cents),
        });
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.amount_cents));
    Ok(out)
}

/// 1099-MISC: gross rents collected per owning entity at or above the threshold.
async fn misc_recipients(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    year: i32,
) -> ApiResult<Vec<Recipient1099>> {
    let llcs: HashMap<Uuid, entity::llc::Model> = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| (l.id, l))
        .collect();
    // property -> owning entity, lease -> property.
    let prop_entity: HashMap<Uuid, Uuid> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|p| p.llc_id.map(|e| (p.id, e)))
        .collect();
    let lease_prop: HashMap<Uuid, Uuid> = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.property_id))
        .collect();

    // Settled, non-deposit rent payments in the year, attributed to the entity.
    let payments = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Status.eq("paid"))
        .filter(entity::lease_payment::Column::Kind.ne(crate::payments::KIND_DEPOSIT))
        .filter(entity::lease_payment::Column::PaidDate.starts_with(format!("{year}-")))
        .all(db)
        .await?;
    let mut by_entity: HashMap<Uuid, i64> = HashMap::new();
    for p in payments {
        if let Some(entity_id) = lease_prop
            .get(&p.lease_id)
            .and_then(|pid| prop_entity.get(pid))
        {
            *by_entity.entry(*entity_id).or_insert(0) += p.amount_cents;
        }
    }

    let mut out: Vec<Recipient1099> = by_entity
        .into_iter()
        .filter(|(_, cents)| *cents >= THRESHOLD_CENTS)
        .map(|(id, cents)| {
            let l = llcs.get(&id);
            Recipient1099 {
                form: "1099-MISC".into(),
                box_label: "Box 1 — Rents".into(),
                recipient_id: id,
                name: l.map(|x| x.name.clone()).unwrap_or_else(|| "—".into()),
                missing_tin: false,
                tin: l.map(|x| x.ein.clone()),
                address: None,
                amount_cents: cents,
                amount_label: usd(cents),
            }
        })
        .collect();
    out.sort_by_key(|r| std::cmp::Reverse(r.amount_cents));
    Ok(out)
}

fn parse_year(year: Option<String>) -> ApiResult<i32> {
    match year.filter(|y| !y.is_empty()) {
        Some(y) => y
            .parse::<i32>()
            .ok()
            .filter(|n| (2000..=2100).contains(n))
            .ok_or_else(|| ApiError::BadRequest("year must be a 4-digit year".into())),
        None => Ok(super::today().year() - 1),
    }
}

async fn build(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    year: i32,
    reveal: bool,
) -> ApiResult<Tax1099Resp> {
    let nec = nec_recipients(db, tenant_id, year, reveal).await?;
    let missing_tin_count = nec.iter().filter(|r| r.missing_tin).count();
    let misc = misc_recipients(db, tenant_id, year).await?;
    let nec_total: i64 = nec.iter().map(|r| r.amount_cents).sum();
    let misc_total: i64 = misc.iter().map(|r| r.amount_cents).sum();
    Ok(Tax1099Resp {
        generated_at: super::today().to_string(),
        year,
        threshold_cents: THRESHOLD_CENTS,
        threshold_label: usd(THRESHOLD_CENTS),
        nec,
        misc,
        nec_total_cents: nec_total,
        nec_total_label: usd(nec_total),
        misc_total_cents: misc_total,
        misc_total_label: usd(misc_total),
        missing_tin_count,
    })
}

fn to_table(r: &Tax1099Resp) -> ReportTable {
    let headers = vec![
        "Form".into(),
        "Recipient".into(),
        "TIN/EIN".into(),
        "Box".into(),
        "Amount".into(),
    ];
    let row = |rec: &Recipient1099| {
        vec![
            rec.form.clone(),
            rec.name.clone(),
            rec.tin.clone().unwrap_or_else(|| {
                if rec.missing_tin {
                    "W-9 missing"
                } else {
                    "—"
                }
                .into()
            }),
            rec.box_label.clone(),
            rec.amount_label.clone(),
        ]
    };
    let mut rows: Vec<Vec<String>> = r.nec.iter().map(row).collect();
    rows.extend(r.misc.iter().map(row));
    let grand = r.nec_total_cents + r.misc_total_cents;

    ReportTable {
        title: format!("1099 tax export — {}", r.year),
        subtitle: Some(format!(
            "Recipients at or above {} · NEC {} · MISC {}",
            r.threshold_label, r.nec_total_label, r.misc_total_label
        )),
        headers,
        rows,
        totals: Some(vec![
            "TOTAL".into(),
            String::new(),
            String::new(),
            String::new(),
            usd(grand),
        ]),
    }
}

/// `GET /reports/1099?<year>` — annual 1099-NEC (vendor) + 1099-MISC (owner
/// rents) recipient totals for the year (defaults to last year).
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/1099?<year>")]
pub async fn tax_1099(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    year: Option<String>,
) -> ApiResult<Json<Tax1099Resp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let y = parse_year(year)?;
    Ok(Json(build(&db, scope.tenant_id, y, false).await?))
}

/// `GET /reports/1099/export?<year>&<format>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/1099/export?<year>&<format>")]
pub async fn tax_1099_export(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    year: Option<String>,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let y = parse_year(year)?;
    let report = build(&db, scope.tenant_id, y, true).await?;
    // The export carries full taxpayer ids, so who pulled it is recorded.
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TAX_1099_EXPORT,
        Some("report"),
        Some(format!("1099-{y}")),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "year": y,
            "recipients": report.nec.len() + report.misc.len(),
            "missing_tin": report.missing_tin_count,
        })),
    )
    .await;
    export(
        &to_table(&report),
        &format!("1099-{y}"),
        format.as_deref().unwrap_or("csv"),
    )
}

#[cfg(test)]
mod tests {
    use super::parse_year;

    #[test]
    fn year_parsing() {
        assert_eq!(parse_year(Some("2025".into())).unwrap(), 2025);
        assert!(parse_year(Some("bad".into())).is_err());
        assert!(parse_year(Some("1999".into())).is_err());
        // Empty / missing defaults to the prior calendar year.
        assert!(parse_year(None).is_ok());
    }
}
