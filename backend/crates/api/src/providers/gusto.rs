//! **Gusto** (payroll) — push approved hours into an unprocessed Gusto payroll,
//! the way Alpha Power Wash does. Push only: Gusto still runs the payroll.
//!
//! The live call reads the company's employees (matched by email), reads the
//! payroll for its `version`, and `PUT`s one entry per person with hourly
//! compensations named "Regular Hours", "Overtime" and "Double overtime".
//! Credentials: the access token in the vault under `gusto.access_token`, the
//! company in the `payroll.gusto_company_uuid` setting. Simulated unless
//! `LIVE_PROVIDERS` lists `gusto`. `GUSTO_API_BASE` switches to the demo host.

use super::{err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

/// One person's hours for the payroll, in hours with three decimals.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct GustoLine {
    pub email: String,
    pub name: String,
    pub regular_hours: String,
    pub overtime_hours: String,
    pub double_overtime_hours: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GustoPush {
    pub company_uuid: String,
    pub payroll_id: String,
    pub lines: Vec<GustoLine>,
}

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct GustoPushResult {
    pub pushed: Vec<String>,
    /// People with hours but no Gusto employee with their email.
    pub unmatched: Vec<String>,
    pub simulated: bool,
}

/// Minutes → Gusto's `"12.500"` hours.
pub fn hours_str(minutes: i64) -> String {
    format!("{}.{:03}", minutes / 60, (minutes % 60) * 1000 / 60)
}

fn base() -> String {
    std::env::var("GUSTO_API_BASE").unwrap_or_else(|_| "https://api.gusto.com".into())
}

pub struct GustoProvider;

#[async_trait::async_trait]
impl Provider for GustoProvider {
    type Request = GustoPush;
    type Response = GustoPushResult;

    fn key(&self) -> &'static str {
        "gusto"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let token = ctx
            .secret("gusto.access_token")
            .await?
            .ok_or_else(|| err("connect Gusto first: no gusto.access_token in the vault"))?;
        let http = super::client::build_http_client()?;
        let get = |url: String| {
            http.get(url)
                .bearer_auth(&token)
                .header("X-Gusto-API-Version", "2024-04-01")
        };
        let employees: serde_json::Value = get(format!(
            "{}/v1/companies/{}/employees",
            base(),
            req.company_uuid
        ))
        .send()
        .await
        .map_err(|e| err(format!("gusto employees request failed: {e}")))?
        .error_for_status()
        .map_err(|e| err(format!("gusto employees: {e}")))?
        .json()
        .await
        .map_err(|e| err(format!("gusto employees: {e}")))?;
        let by_email: HashMap<String, String> = employees
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some((
                    e.get("email")?.as_str()?.to_lowercase(),
                    e.get("uuid")?.as_str()?.to_string(),
                ))
            })
            .collect();
        let payroll: serde_json::Value = get(format!(
            "{}/v1/companies/{}/payrolls/{}",
            base(),
            req.company_uuid,
            req.payroll_id
        ))
        .send()
        .await
        .map_err(|e| err(format!("gusto payroll request failed: {e}")))?
        .error_for_status()
        .map_err(|e| err(format!("gusto payroll: {e}")))?
        .json()
        .await
        .map_err(|e| err(format!("gusto payroll: {e}")))?;
        if payroll.get("processed").and_then(|v| v.as_bool()) == Some(true) {
            return Err(err("that Gusto payroll is already processed"));
        }
        let version = payroll.get("version").cloned().unwrap_or(json!(null));
        let (mut comps, mut pushed, mut unmatched) = (Vec::new(), Vec::new(), Vec::new());
        for l in &req.lines {
            match by_email.get(&l.email.to_lowercase()) {
                Some(uuid) => {
                    comps.push(json!({
                        "employee_uuid": uuid,
                        "hourly_compensations": [
                            { "name": "Regular Hours", "hours": l.regular_hours },
                            { "name": "Overtime", "hours": l.overtime_hours },
                            { "name": "Double overtime", "hours": l.double_overtime_hours },
                        ],
                    }));
                    pushed.push(l.name.clone());
                }
                None => unmatched.push(l.name.clone()),
            }
        }
        http.put(format!(
            "{}/v1/companies/{}/payrolls/{}",
            base(),
            req.company_uuid,
            req.payroll_id
        ))
        .bearer_auth(&token)
        .header("X-Gusto-API-Version", "2024-04-01")
        .json(&json!({ "version": version, "employee_compensations": comps }))
        .send()
        .await
        .map_err(|e| err(format!("gusto payroll update failed: {e}")))?
        .error_for_status()
        .map_err(|e| err(format!("gusto payroll update: {e}")))?;
        Ok(GustoPushResult {
            pushed,
            unmatched,
            simulated: false,
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        Ok(GustoPushResult {
            pushed: req.lines.iter().map(|l| l.name.clone()).collect(),
            unmatched: vec![],
            simulated: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gusto_hours_have_three_decimals() {
        assert_eq!(hours_str(2_400), "40.000");
        assert_eq!(hours_str(90), "1.500");
        assert_eq!(hours_str(1), "0.016");
        assert_eq!(hours_str(0), "0.000");
    }
}
