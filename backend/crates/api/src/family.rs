//! **Family-plan features** (roadmap area 17).
//!
//! * **Related-party guard**: a counterparty linked to one of the family's own
//!   LLCs or owners is a related party. A bill from one gets a review that
//!   needs a market-rate note and a decision by someone who isn't a party
//!   (an owner of the receiving side, or whoever raised it) before the bill
//!   can be approved.
//! * **Foundation mode** (per LLC): income certifications against a percent
//!   of area median income, housing vouchers that split each month's rent
//!   into the resident's share and the housing authority's (HAP), and an
//!   at-cost management fee: staff time on its properties at pay rate plus
//!   burden, instead of a percent of rent.

use crate::error::{ApiError, ApiResult};
use chrono::{NaiveDate, Utc};
use entity::prelude::{
    Counterparty, EntityOwnership, Llc, MaintenanceTicket, Owner, Property, RelatedPartyReview,
    TimeEntry,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Is a household's income within the limit (`limit_pct` of AMI)?
pub fn qualifies(annual_income_cents: i64, ami_cents: i64, limit_pct: i32) -> bool {
    annual_income_cents as i128 * 100 <= ami_cents as i128 * limit_pct as i128
}

/// Split a month's rent between the resident and the housing authority:
/// `(resident share, HAP)`. The HAP never exceeds the rent.
pub fn split_hap(amount_cents: i64, hap_cents: i64) -> (i64, i64) {
    let hap = hap_cents.clamp(0, amount_cents.max(0));
    (amount_cents - hap, hap)
}

/// Does a voucher cover `day`?
pub fn voucher_covers(v: &entity::housing_voucher::Model, day: NaiveDate) -> bool {
    v.starts_on <= day && v.ends_on.is_none_or(|e| day <= e)
}

/// Where an income certification stands on `today`.
pub fn cert_state(
    c: Option<&entity::income_certification::Model>,
    today: NaiveDate,
) -> &'static str {
    match c {
        None => "missing",
        Some(c) if !c.qualified => "over_limit",
        Some(c) if c.expires_on < today => "expired",
        Some(c) if (c.expires_on - today).num_days() <= 60 => "expiring",
        Some(_) => "ok",
    }
}

/// Why `decider` may not decide a review, if they may not: they are one of its
/// parties (by their owner record) or raised it.
pub fn bar_to_deciding(
    decider_owner_ids: &[Uuid],
    parties: &[Uuid],
    decider_user: Uuid,
    raised_by: Option<Uuid>,
) -> Option<&'static str> {
    if decider_owner_ids.iter().any(|o| parties.contains(o)) {
        return Some("You're a party to this, so someone else has to decide it.");
    }
    if raised_by == Some(decider_user) {
        return Some("You raised this, so someone else has to decide it.");
    }
    None
}

pub fn parties_of(r: &entity::related_party_review::Model) -> Vec<Uuid> {
    serde_json::from_value(r.parties.clone()).unwrap_or_default()
}

/// Is this counterparty one of the family's own?
pub fn is_related(c: &entity::counterparty::Model) -> bool {
    c.related_llc_id.is_some() || c.related_owner_id.is_some()
}

/// The owners on a related counterparty's side: the linked owner and the
/// owners of the linked LLC.
pub async fn parties_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    c: &entity::counterparty::Model,
) -> ApiResult<Vec<Uuid>> {
    let mut out: Vec<Uuid> = c.related_owner_id.into_iter().collect();
    if let Some(llc) = c.related_llc_id {
        for o in EntityOwnership::find()
            .filter(entity::entity_ownership::Column::TenantId.eq(tenant_id))
            .filter(entity::entity_ownership::Column::EntityId.eq(llc))
            .all(db)
            .await?
        {
            if !out.contains(&o.owner_id) {
                out.push(o.owner_id);
            }
        }
    }
    Ok(out)
}

/// The owner records a user is linked to.
pub async fn owner_ids_of(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<Vec<Uuid>> {
    Ok(Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .filter(entity::owner::Column::UserId.eq(user_id))
        .all(db)
        .await?
        .into_iter()
        .map(|o| o.id)
        .collect())
}

/// The review for a bill from a related party, made if it's missing. `None`
/// when the vendor isn't related.
pub async fn ensure_bill_review(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    bill: &entity::vendor_bill::Model,
    actor: Option<Uuid>,
) -> ApiResult<Option<entity::related_party_review::Model>> {
    let Some(c) = Counterparty::find_by_id(bill.counterparty_id)
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if !is_related(&c) {
        return Ok(None);
    }
    if let Some(r) = RelatedPartyReview::find()
        .filter(entity::related_party_review::Column::TenantId.eq(tenant_id))
        .filter(entity::related_party_review::Column::SubjectType.eq("vendor_bill"))
        .filter(entity::related_party_review::Column::SubjectId.eq(bill.id))
        .one(db)
        .await?
    {
        return Ok(Some(r));
    }
    let parties = parties_for(db, tenant_id, &c).await?;
    let reason = match (c.related_llc_id, c.related_owner_id) {
        (Some(_), _) => format!("{} is one of the family's own entities.", c.name),
        _ => format!("{} is a family member.", c.name),
    };
    let now = Utc::now();
    let r = entity::related_party_review::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        subject_type: Set("vendor_bill".into()),
        subject_id: Set(Some(bill.id)),
        entity_id: Set(Some(bill.entity_id)),
        counterparty_id: Set(Some(c.id)),
        summary: Set(format!("Bill {} from {}", bill.bill_number, c.name)),
        reason: Set(reason),
        amount_cents: Set(Some(bill.amount_cents)),
        market_cents: Set(None),
        market_note: Set(None),
        parties: Set(serde_json::json!(parties)),
        status: Set("open".into()),
        decided_by: Set(None),
        decided_at: Set(None),
        decision_note: Set(None),
        created_by: Set(bill.created_by.or(actor)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    crate::audit::record(
        db,
        actor,
        crate::audit::actions::RELATED_PARTY_FLAG,
        Some("related_party_review"),
        Some(r.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "subject": "vendor_bill", "bill_id": bill.id, "auto": true })),
    )
    .await;
    Ok(Some(r))
}

/// Before a bill is approved: a related-party bill needs its review approved,
/// and a party to it can't approve the bill either.
pub async fn guard_bill(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    bill: &entity::vendor_bill::Model,
    user_id: Uuid,
) -> ApiResult<()> {
    let Some(r) = ensure_bill_review(db, tenant_id, bill, Some(user_id)).await? else {
        return Ok(());
    };
    if r.status != "approved" {
        return Err(ApiError::BadRequest(
            "This bill is from a related party. It needs a market-rate note and approval \
             from someone who isn't a party first (Related parties)."
                .into(),
        ));
    }
    let mine = owner_ids_of(db, tenant_id, user_id).await?;
    if mine.iter().any(|o| parties_of(&r).contains(o)) {
        return Err(ApiError::Forbidden(
            "You're a party to this bill, so someone else has to approve it.".into(),
        ));
    }
    Ok(())
}

/// The at-cost management fee for an LLC over a period: staff time on its
/// properties (directly, or on their work orders) at pay rate, plus the labor
/// burden for employees. Time already billed to the owner on an in-house bill
/// is an expense on the books already, so it's left out.
pub async fn at_cost_fee(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    llc_id: Uuid,
    period_start: &str,
    period_end: &str,
) -> ApiResult<i64> {
    let (Ok(start), Ok(end)) = (
        NaiveDate::parse_from_str(period_start, "%Y-%m-%d"),
        NaiveDate::parse_from_str(period_end, "%Y-%m-%d"),
    ) else {
        return Ok(0);
    };
    let props: Vec<Uuid> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::LlcId.eq(llc_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    if props.is_empty() {
        return Ok(0);
    }
    let tickets: Vec<Uuid> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.is_in(props.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::BilledBillId.is_null())
        .filter(entity::time_entry::Column::EndedAt.is_not_null())
        .filter(
            sea_orm::Condition::any()
                .add(entity::time_entry::Column::PropertyId.is_in(props))
                .add(entity::time_entry::Column::MaintenanceTicketId.is_in(tickets)),
        )
        .all(db)
        .await?;
    let rules = crate::workforce::Rules::load(db, tenant_id).await;
    let profiles = crate::workforce::profiles_by_user(db, tenant_id).await?;
    let now = Utc::now();
    let mut total = 0i64;
    for e in entries {
        let day = e.started_at.with_timezone(&rules.tz).date_naive();
        if day < start || day > end {
            continue;
        }
        let prof = profiles.get(&e.user_id);
        let rate = e
            .pay_rate_cents
            .or(prof.map(|p| p.pay_rate_cents))
            .unwrap_or(0);
        let pay = crate::workforce::minutes_cost(crate::workforce::entry_minutes(&e, now), rate);
        let contractor = prof.is_some_and(|p| p.employment_type == "contractor");
        let burden = if contractor {
            0
        } else {
            (pay * rules.burden_bps + 5_000) / 10_000
        };
        total += pay + burden;
    }
    Ok(total)
}

/// The management fee for an LLC's period: a percent of collected rent, or at
/// cost for an LLC set that way. Returns `(fee, basis)`.
pub async fn mgmt_fee(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    llc_id: Uuid,
    period_start: &str,
    period_end: &str,
    rent_collected_cents: i64,
) -> ApiResult<(i64, &'static str)> {
    let basis = Llc::find_by_id(llc_id)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|l| l.fee_basis)
        .unwrap_or_default();
    if basis == "at_cost" {
        let fee = at_cost_fee(db, tenant_id, llc_id, period_start, period_end).await?;
        return Ok((fee, "at_cost"));
    }
    let bps = crate::settings::get_i64(db, tenant_id, crate::settings::PAYOUT_MGMT_FEE_BPS).await;
    let fee = crate::payouts::compute_amounts(rent_collected_cents, 0, bps).mgmt_fee_cents;
    Ok((fee, "percent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn income_limits() {
        // 60% of an $80,000 AMI is $48,000.
        assert!(qualifies(4_800_000, 8_000_000, 60));
        assert!(!qualifies(4_800_001, 8_000_000, 60));
        assert!(qualifies(0, 8_000_000, 30));
    }

    #[test]
    fn hap_split() {
        assert_eq!(split_hap(150_000, 110_000), (40_000, 110_000));
        // A voucher bigger than the rent covers the rent and no more.
        assert_eq!(split_hap(100_000, 120_000), (0, 100_000));
        assert_eq!(split_hap(100_000, 0), (100_000, 0));
    }

    #[test]
    fn deciding() {
        let (me, fam) = (Uuid::new_v4(), Uuid::new_v4());
        let other = Uuid::new_v4();
        assert!(bar_to_deciding(&[fam], &[fam], me, None).is_some());
        assert!(bar_to_deciding(&[], &[fam], me, Some(me)).is_some());
        assert!(bar_to_deciding(&[other], &[fam], me, Some(Uuid::new_v4())).is_none());
    }

    #[test]
    fn cert_states() {
        let today = d("2026-10-04");
        let mut c = entity::income_certification::Model {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            lease_id: Uuid::nil(),
            effective_on: d("2026-01-01"),
            expires_on: d("2027-01-01"),
            household_size: 3,
            annual_income_cents: 1,
            ami_cents: 2,
            limit_pct: 60,
            qualified: true,
            notes: None,
            certified_by: None,
            created_at: Utc::now().into(),
        };
        assert_eq!(cert_state(None, today), "missing");
        assert_eq!(cert_state(Some(&c), today), "ok");
        c.expires_on = d("2026-11-15");
        assert_eq!(cert_state(Some(&c), today), "expiring");
        c.expires_on = d("2026-09-30");
        assert_eq!(cert_state(Some(&c), today), "expired");
        c.qualified = false;
        assert_eq!(cert_state(Some(&c), today), "over_limit");
    }
}
