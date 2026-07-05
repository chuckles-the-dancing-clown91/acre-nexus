//! **Bank feeds & reconciliation** (roadmap Phase 3, issue #36).
//!
//! A linked [`entity::bank_account`] (Plaid live, simulated otherwise) syncs
//! its transactions through the durable `bank_feed_sync` job. Incoming lines
//! upsert by provider transaction id (re-syncs dedupe), then **auto-match**:
//! an unmatched deposit reconciles against a settled payment when the amount
//! is exact, the dates are within a few days, and the payment belongs to the
//! account's entity. What doesn't match stays `unmatched` for the console's
//! manual match / ignore actions — reconciliation is a review queue, not a
//! guess.

use crate::modules::JobOutcome;
use crate::providers::bank::{BankRequest, BankResponse, ExpectedDeposit, PlaidProvider};
use crate::providers::ProviderCtx;
use chrono::{NaiveDate, Utc};
use entity::prelude::{BankAccount, BankTxn, Lease, LeasePayment, Property};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

/// Days of history each sync asks for.
const SYNC_WINDOW_DAYS: i64 = 60;
/// Auto-match tolerance between the payment date and the bank posted date.
const MATCH_WINDOW_DAYS: i64 = 3;

/// Advance one `bank_feed_sync` job: pull the feed, upsert lines, auto-match.
pub async fn handle_sync_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let Some(account_id) = job
        .payload
        .get("bank_account_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("bank_feed_sync payload missing bank_account_id");
    };
    let account = match BankAccount::find_by_id(account_id)
        .filter(entity::bank_account::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(a)) => a,
        Ok(None) => return JobOutcome::failed("bank account not found"),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };
    let Some(external_id) = account.external_id.clone() else {
        return JobOutcome::failed("bank account is not linked for feeds");
    };

    let since = (Utc::now().date_naive() - chrono::Duration::days(SYNC_WINDOW_DAYS)).to_string();

    // The settled payments the ledger expects to have landed in this account's
    // entity — they drive the simulator and the matcher alike.
    let candidates =
        match settled_payments_for_entity(db, job.tenant_id, account.entity_id, &since).await {
            Ok(c) => c,
            Err(e) => {
                return JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("db error: {e}"),
                )
            }
        };
    let expected: Vec<ExpectedDeposit> = candidates
        .iter()
        .filter_map(|p| {
            Some(ExpectedDeposit {
                date: p.paid_date.clone()?,
                amount_cents: p.amount_cents,
                memo: format!(
                    "ACH DEPOSIT {}",
                    p.receipt_number.as_deref().unwrap_or("RENT")
                ),
            })
        })
        .collect();

    let ctx = ProviderCtx::new(db, job.tenant_id);
    let req = BankRequest::Sync {
        bank_account_id: account.id,
        account_external_id: external_id,
        since: since.clone(),
        expected,
    };
    let lines = match crate::providers::run(&PlaidProvider, &ctx, job, &req).await {
        Ok(BankResponse::Transactions { lines }) => lines,
        Ok(BankResponse::Linked { .. }) => {
            return JobOutcome::failed("provider returned Linked for a Sync request")
        }
        Err(outcome) => return outcome,
    };

    // Upsert by (account, external_id).
    let mut inserted = 0;
    for line in &lines {
        let exists = BankTxn::find()
            .filter(entity::bank_txn::Column::TenantId.eq(job.tenant_id))
            .filter(entity::bank_txn::Column::BankAccountId.eq(account.id))
            .filter(entity::bank_txn::Column::ExternalId.eq(line.external_id.clone()))
            .one(db)
            .await;
        match exists {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(e) => {
                return JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("db error: {e}"),
                )
            }
        }
        let row = entity::bank_txn::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(job.tenant_id),
            bank_account_id: Set(account.id),
            posted_date: Set(line.posted_date.clone()),
            description: Set(line.description.clone()),
            amount_cents: Set(line.amount_cents),
            external_id: Set(line.external_id.clone()),
            status: Set("unmatched".into()),
            matched_payment_id: Set(None),
            created_at: Set(Utc::now().into()),
        };
        if let Err(e) = row.insert(db).await {
            tracing::error!("bank_feed_sync: insert failed: {e}");
            continue;
        }
        inserted += 1;
    }

    // Auto-match the account's unmatched deposits.
    let unmatched = BankTxn::find()
        .filter(entity::bank_txn::Column::TenantId.eq(job.tenant_id))
        .filter(entity::bank_txn::Column::BankAccountId.eq(account.id))
        .filter(entity::bank_txn::Column::Status.eq("unmatched"))
        .all(db)
        .await
        .unwrap_or_default();
    let already_matched: Vec<Uuid> = BankTxn::find()
        .filter(entity::bank_txn::Column::TenantId.eq(job.tenant_id))
        .filter(entity::bank_txn::Column::Status.eq("matched"))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| t.matched_payment_id)
        .collect();
    let available: Vec<&entity::lease_payment::Model> = candidates
        .iter()
        .filter(|p| !already_matched.contains(&p.id))
        .collect();

    let pairs = auto_match(&unmatched, &available);
    let mut matched = 0;
    for (txn_id, payment_id) in &pairs {
        let Ok(Some(txn)) = BankTxn::find_by_id(*txn_id).one(db).await else {
            continue;
        };
        let mut am: entity::bank_txn::ActiveModel = txn.into();
        am.status = Set("matched".into());
        am.matched_payment_id = Set(Some(*payment_id));
        if am.update(db).await.is_ok() {
            matched += 1;
            crate::audit::record(
                db,
                None,
                crate::audit::actions::BANK_TXN_MATCH,
                Some("bank_txn"),
                Some(txn_id.to_string()),
                Some(job.tenant_id),
                Some(json!({ "payment_id": payment_id, "auto": true })),
            )
            .await;
        }
    }

    // Stamp the sync.
    let mut am: entity::bank_account::ActiveModel = account.clone().into();
    am.last_synced_at = Set(Some(Utc::now().into()));
    let _ = am.update(db).await;

    crate::audit::record(
        db,
        None,
        crate::audit::actions::BANK_FEED_SYNC,
        Some("bank_account"),
        Some(account.id.to_string()),
        Some(job.tenant_id),
        Some(json!({ "lines": lines.len(), "inserted": inserted, "auto_matched": matched })),
    )
    .await;

    JobOutcome::completed(json!({
        "lines": lines.len(),
        "inserted": inserted,
        "auto_matched": matched,
    }))
}

/// Settled electronic payments on the entity's properties since `since` —
/// the reconciliation candidates.
pub async fn settled_payments_for_entity(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entity_id: Uuid,
    since: &str,
) -> Result<Vec<entity::lease_payment::Model>, sea_orm::DbErr> {
    let property_ids: Vec<Uuid> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::LlcId.eq(entity_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    if property_ids.is_empty() {
        return Ok(vec![]);
    }
    let lease_ids: Vec<Uuid> = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::PropertyId.is_in(property_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|l| l.id)
        .collect();
    if lease_ids.is_empty() {
        return Ok(vec![]);
    }
    LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::LeaseId.is_in(lease_ids))
        .filter(entity::lease_payment::Column::Status.eq("paid"))
        .filter(entity::lease_payment::Column::PaidDate.gte(since))
        .all(db)
        .await
}

/// Pure matcher: pair unmatched **deposits** with settled payments on exact
/// amount + date proximity. Greedy, one payment per line, deterministic
/// (lines in feed order, candidates by closest date).
pub fn auto_match(
    lines: &[entity::bank_txn::Model],
    payments: &[&entity::lease_payment::Model],
) -> Vec<(Uuid, Uuid)> {
    let mut used: Vec<Uuid> = Vec::new();
    let mut pairs = Vec::new();
    for line in lines {
        if line.amount_cents <= 0 {
            continue; // withdrawals never match receivables
        }
        let Ok(posted) = NaiveDate::parse_from_str(&line.posted_date, "%Y-%m-%d") else {
            continue;
        };
        let best = payments
            .iter()
            .filter(|p| !used.contains(&p.id))
            .filter(|p| p.amount_cents == line.amount_cents)
            .filter_map(|p| {
                let paid = p.paid_date.as_deref()?;
                let paid = NaiveDate::parse_from_str(paid, "%Y-%m-%d").ok()?;
                let gap = (posted - paid).num_days().abs();
                (gap <= MATCH_WINDOW_DAYS).then_some((gap, *p))
            })
            .min_by_key(|(gap, _)| *gap);
        if let Some((_, payment)) = best {
            used.push(payment.id);
            pairs.push((line.id, payment.id));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u128, date: &str, amount: i64) -> entity::bank_txn::Model {
        entity::bank_txn::Model {
            id: Uuid::from_u128(id),
            tenant_id: Uuid::from_u128(1),
            bank_account_id: Uuid::from_u128(2),
            posted_date: date.into(),
            description: "ACH".into(),
            amount_cents: amount,
            external_id: format!("ext{id}"),
            status: "unmatched".into(),
            matched_payment_id: None,
            created_at: chrono::Utc::now().into(),
        }
    }

    fn payment(id: u128, paid: &str, amount: i64) -> entity::lease_payment::Model {
        entity::lease_payment::Model {
            id: Uuid::from_u128(id),
            tenant_id: Uuid::from_u128(1),
            lease_id: Uuid::from_u128(3),
            due_date: paid.into(),
            amount_cents: amount,
            paid_date: Some(paid.into()),
            status: "paid".into(),
            method: Some("ach".into()),
            created_at: chrono::Utc::now().into(),
            kind: "rent".into(),
            method_id: None,
            provider: None,
            external_id: None,
            failure_reason: None,
            receipt_number: None,
            ledger_txn_id: None,
        }
    }

    #[test]
    fn matches_exact_amount_within_window() {
        let lines = vec![line(10, "2026-06-03", 185_000)];
        let p = payment(20, "2026-06-01", 185_000);
        let pairs = auto_match(&lines, &[&p]);
        assert_eq!(pairs, vec![(Uuid::from_u128(10), Uuid::from_u128(20))]);
    }

    #[test]
    fn ignores_wrong_amount_far_dates_and_withdrawals() {
        let p = payment(20, "2026-06-01", 185_000);
        // Wrong amount.
        assert!(auto_match(&[line(10, "2026-06-01", 185_001)], &[&p]).is_empty());
        // Too far apart.
        assert!(auto_match(&[line(11, "2026-06-10", 185_000)], &[&p]).is_empty());
        // Withdrawals (negative) never match.
        assert!(auto_match(&[line(12, "2026-06-01", -185_000)], &[&p]).is_empty());
    }

    #[test]
    fn one_payment_matches_at_most_one_line() {
        let lines = vec![
            line(10, "2026-06-02", 185_000),
            line(11, "2026-06-03", 185_000),
        ];
        let p = payment(20, "2026-06-01", 185_000);
        let pairs = auto_match(&lines, &[&p]);
        // The closer line wins; the other stays unmatched.
        assert_eq!(pairs, vec![(Uuid::from_u128(10), Uuid::from_u128(20))]);
    }

    #[test]
    fn prefers_the_closest_date() {
        let lines = vec![line(10, "2026-06-03", 185_000)];
        let far = payment(20, "2026-06-06", 185_000);
        let near = payment(21, "2026-06-03", 185_000);
        let pairs = auto_match(&lines, &[&far, &near]);
        assert_eq!(pairs, vec![(Uuid::from_u128(10), Uuid::from_u128(21))]);
    }
}
