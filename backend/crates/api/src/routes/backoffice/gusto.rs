//! Payroll hand-off to Gusto: the hours sheet (JSON / CSV, also the manual
//! fallback) and the push into an unprocessed Gusto payroll. `payroll:read`.
//!
//! Only approved time goes; missed punches waiting on the office and anyone
//! without an email are left out and listed. Weeks are whole Monday–Sunday
//! weeks (a week straddling two pay periods belongs to the one it ends in), so
//! overtime is never split across payrolls.

use super::reports::payroll_data;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::providers::gusto::{hours_str, GustoLine, GustoProvider, GustoPush, GustoPushResult};
use crate::providers::{Provider, ProviderCtx};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::tenancy::TenantScope;
use crate::workforce::{overtime, parse_date, Rules};
use chrono::{Duration, NaiveDate};
use entity::prelude::User;
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct GustoHours {
    /// Whole weeks covered: the Mondays from `from`'s week to the week ending
    /// on or before `to`.
    pub from: String,
    pub to: String,
    pub lines: Vec<GustoLine>,
    pub left_out: Vec<String>,
}

/// Pay-period weeks: every Monday–Sunday week that *ends* inside [from, to].
fn period_weeks(from: NaiveDate, to: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first_end = overtime::week_start(from) + Duration::days(6);
    let first = if first_end < from {
        overtime::week_start(from) + Duration::days(7)
    } else {
        overtime::week_start(from)
    };
    let last_monday = overtime::week_start(to - Duration::days(6));
    (first, last_monday.max(first))
}

async fn hours(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    rules: &Rules,
) -> ApiResult<GustoHours> {
    let (first, last) = period_weeks(from, to);
    let r = payroll_data(db, tenant_id, first, last, true, rules).await?;
    let emails: HashMap<Uuid, String> = User::find()
        .filter(
            entity::user::Column::Id.is_in(r.rows.iter().map(|x| x.user_id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.email))
        .collect();
    let mut per: BTreeMap<Uuid, (String, i64, i64, i64)> = BTreeMap::new();
    for row in r.rows.iter().filter(|x| {
        let m: NaiveDate = x.week_of.parse().unwrap_or(first);
        m >= first && m <= last
    }) {
        let e = per
            .entry(row.user_id)
            .or_insert((row.name.clone(), 0, 0, 0));
        e.1 += row.regular_minutes;
        e.2 += row.overtime_minutes;
        e.3 += row.double_minutes;
    }
    let mut left_out = r.excluded;
    let mut lines = Vec::new();
    for (uid, (name, reg, ot, dt)) in per {
        match emails.get(&uid) {
            Some(email) => lines.push(GustoLine {
                email: email.clone(),
                name,
                regular_hours: hours_str(reg),
                overtime_hours: hours_str(ot),
                double_overtime_hours: hours_str(dt),
            }),
            None => left_out.push(format!("{name} — no email to match in Gusto")),
        }
    }
    Ok(GustoHours {
        from: first.to_string(),
        to: (last + Duration::days(6)).to_string(),
        lines,
        left_out,
    })
}

/// `GET /payroll/gusto/hours?from&to` — what would go to Gusto.
#[rocket_okapi::openapi(tag = "Payroll")]
#[get("/payroll/gusto/hours?<from>&<to>")]
pub async fn gusto_hours(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: String,
    to: String,
) -> ApiResult<Json<GustoHours>> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = (parse_date(&from, "from")?, parse_date(&to, "to")?);
    Ok(Json(hours(&db, scope.tenant_id, f, t, &rules).await?))
}

/// `GET /payroll/gusto/hours.csv?from&to` — the same, for Gusto's hours import.
#[rocket_okapi::openapi(skip)]
#[get("/payroll/gusto/hours.csv?<from>&<to>")]
pub async fn gusto_hours_csv(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: String,
    to: String,
) -> ApiResult<ReportFile> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = (parse_date(&from, "from")?, parse_date(&to, "to")?);
    let h = hours(&db, scope.tenant_id, f, t, &rules).await?;
    let mut csv = String::from("email,name,regular_hours,overtime_hours,double_overtime_hours\n");
    for l in &h.lines {
        csv.push_str(&format!(
            "{},\"{}\",{},{},{}\n",
            l.email,
            l.name.replace('"', "\"\""),
            l.regular_hours,
            l.overtime_hours,
            l.double_overtime_hours
        ));
    }
    Ok(ReportFile::new(
        csv.into_bytes(),
        "text/csv",
        format!("gusto-hours-{}.csv", h.from),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct GustoStatus {
    pub company_uuid: Option<String>,
    pub live: bool,
}

/// `GET /payroll/gusto/status`.
#[rocket_okapi::openapi(tag = "Payroll")]
#[get("/payroll/gusto/status")]
pub async fn gusto_status(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<GustoStatus>> {
    user.require(Permission::PayrollRead)?;
    let c = crate::settings::get_string(
        &db,
        scope.tenant_id,
        crate::settings::PAYROLL_GUSTO_COMPANY_UUID,
    )
    .await;
    Ok(Json(GustoStatus {
        company_uuid: Some(c).filter(|s| !s.trim().is_empty()),
        live: crate::providers::is_live("gusto"),
    }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GustoPushReq {
    /// The unprocessed Gusto payroll to fill.
    pub payroll_id: String,
    pub from: String,
    pub to: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct GustoPushResp {
    #[serde(flatten)]
    pub result: GustoPushResult,
    pub left_out: Vec<String>,
}

/// `POST /payroll/gusto/push` — fill an unprocessed Gusto payroll with the
/// period's approved hours. The office still reviews and runs it in Gusto.
#[rocket_okapi::openapi(tag = "Payroll")]
#[post("/payroll/gusto/push", data = "<body>")]
pub async fn gusto_push(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<GustoPushReq>,
) -> ApiResult<Json<GustoPushResp>> {
    user.require(Permission::PayrollRead)?;
    user.require(Permission::TeamManage)?;
    let company = crate::settings::get_string(
        &db,
        scope.tenant_id,
        crate::settings::PAYROLL_GUSTO_COMPANY_UUID,
    )
    .await;
    if company.trim().is_empty() && crate::providers::is_live("gusto") {
        return Err(ApiError::BadRequest(
            "set the Gusto company ID in Settings first".into(),
        ));
    }
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = (parse_date(&body.from, "from")?, parse_date(&body.to, "to")?);
    let h = hours(&db, scope.tenant_id, f, t, &rules).await?;
    let ctx = ProviderCtx::new(&db, scope.tenant_id);
    let result = GustoProvider
        .execute(
            &ctx,
            &GustoPush {
                company_uuid: company,
                payroll_id: body.payroll_id.trim().to_string(),
                lines: h.lines,
            },
        )
        .await
        .map_err(|e| ApiError::BadRequest(e.0))?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PAYROLL_GUSTO_PUSH,
        Some("payroll"),
        Some(body.payroll_id.clone()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "from": h.from, "to": h.to,
            "people": result.pushed.len(), "unmatched": result.unmatched.len(),
            "simulated": result.simulated,
        })),
    )
    .await;
    Ok(Json(GustoPushResp {
        result,
        left_out: h.left_out,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_straddling_week_belongs_to_the_period_it_ends_in() {
        let d = |s: &str| s.parse::<NaiveDate>().unwrap();
        // Pay period Sep 1 (Tue) – Sep 15 (Tue): the week of Aug 31 ends Sep 6
        // (in), the week of Sep 14 ends Sep 20 (next period).
        let (first, last) = period_weeks(d("2026-09-01"), d("2026-09-15"));
        assert_eq!(first, d("2026-08-31"));
        assert_eq!(last, d("2026-09-07"));
        // A period starting on a Monday takes that week.
        let (first, _) = period_weeks(d("2026-09-14"), d("2026-09-27"));
        assert_eq!(first, d("2026-09-14"));
    }
}
