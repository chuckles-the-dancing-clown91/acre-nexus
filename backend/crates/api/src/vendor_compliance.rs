//! **Vendor compliance** (fix plan F11–F12): the W-9 taxpayer id the 1099
//! needs, and insurance certificates with expiry. The rules are pure and
//! unit-tested; TINs are encrypted with the PII key and only the last four are
//! shown, except in the 1099 export that has to carry them.

use chrono::NaiveDate;
use entity::prelude::{VendorInsurance, VendorTaxProfile};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

pub const CLASSIFICATIONS: &[&str] = &[
    "individual",
    "c_corp",
    "s_corp",
    "partnership",
    "trust",
    "llc_c",
    "llc_s",
    "llc_p",
    "other",
];
pub const INSURANCE_KINDS: &[&str] = &[
    "general_liability",
    "workers_comp",
    "auto",
    "umbrella",
    "professional",
];
/// A certificate inside this many days of its end is "expiring".
pub const EXPIRING_DAYS: i64 = 30;

/// Validate and normalise a taxpayer id: nine digits, shaped like a real SSN or
/// EIN. Returns the bare digits.
pub fn validate_tin(tin_type: &str, raw: &str) -> Result<String, String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 9
        || raw
            .chars()
            .any(|c| !(c.is_ascii_digit() || c == '-' || c == ' '))
    {
        return Err("a taxpayer id is nine digits".into());
    }
    match tin_type {
        "ssn" => {
            let area: u32 = digits[0..3].parse().unwrap_or(0);
            let group: u32 = digits[3..5].parse().unwrap_or(0);
            let serial: u32 = digits[5..9].parse().unwrap_or(0);
            if area == 0 || area == 666 || area >= 900 || group == 0 || serial == 0 {
                return Err("that is not a valid SSN".into());
            }
        }
        "ein" => {
            // Prefixes the IRS assigns (campus codes).
            const VALID: &[u32] = &[
                1, 2, 3, 4, 5, 6, 10, 11, 12, 13, 14, 15, 16, 20, 21, 22, 23, 24, 25, 26, 27, 30,
                31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 50, 51, 52,
                53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 71, 72, 73, 74, 75,
                76, 77, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90, 91, 92, 93, 94, 95, 98, 99,
            ];
            let prefix: u32 = digits[0..2].parse().unwrap_or(0);
            if !VALID.contains(&prefix) {
                return Err("that is not a valid EIN".into());
            }
        }
        _ => return Err("tin_type must be ssn or ein".into()),
    }
    Ok(digits)
}

/// The full id in its usual shape (for the 1099 export only).
pub fn format_tin(tin_type: &str, digits: &str) -> String {
    if digits.len() != 9 {
        return digits.to_string();
    }
    match tin_type {
        "ssn" => format!("{}-{}-{}", &digits[0..3], &digits[3..5], &digits[5..9]),
        _ => format!("{}-{}", &digits[0..2], &digits[2..9]),
    }
}

/// The id as shown on screen: only the last four.
pub fn mask_tin(tin_type: &str, last4: &str) -> String {
    match tin_type {
        "ssn" => format!("•••-••-{last4}"),
        _ => format!("••-•••{last4}"),
    }
}

/// `current` | `expiring` | `expired` for a certificate ending on `expires_on`.
pub fn insurance_state(expires_on: &str, today: NaiveDate) -> &'static str {
    match NaiveDate::parse_from_str(expires_on, "%Y-%m-%d") {
        Ok(d) if d < today => "expired",
        Ok(d) if (d - today).num_days() <= EXPIRING_DAYS => "expiring",
        Ok(_) => "current",
        Err(_) => "expired",
    }
}

/// Whether the vendor has general liability cover in force today.
pub async fn coi_current(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    counterparty_id: Uuid,
    today: NaiveDate,
) -> Result<bool, sea_orm::DbErr> {
    Ok(VendorInsurance::find()
        .filter(entity::vendor_insurance::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_insurance::Column::CounterpartyId.eq(counterparty_id))
        .filter(entity::vendor_insurance::Column::Kind.eq("general_liability"))
        .all(db)
        .await?
        .iter()
        .any(|p| insurance_state(&p.expires_on, today) != "expired"))
}

/// The vendor's decrypted TIN, formatted, for the 1099 export.
pub async fn reveal_tin(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    counterparty_id: Uuid,
) -> Option<String> {
    let p = VendorTaxProfile::find()
        .filter(entity::vendor_tax_profile::Column::TenantId.eq(tenant_id))
        .filter(entity::vendor_tax_profile::Column::CounterpartyId.eq(counterparty_id))
        .one(db)
        .await
        .ok()
        .flatten()?;
    let key = &crate::config::Config::global().pii_key;
    let digits = crate::pii::decrypt(key, &p.tin_ciphertext, &p.tin_nonce).ok()?;
    Some(format_tin(&p.tin_type, &digits))
}

/// Whether to require current insurance before a vendor is sent out.
pub async fn require_coi(db: &impl ConnectionTrait, tenant_id: Uuid) -> bool {
    crate::settings::get_bool(db, tenant_id, crate::settings::COMPLIANCE_REQUIRE_COI).await
}

/// The dispatch gate: with the setting on, a vendor without current general
/// liability cover is only assigned with a reason, which is audited.
pub async fn check_dispatch(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    counterparty_id: Uuid,
    override_reason: Option<&str>,
    actor: Option<Uuid>,
    ticket_id: Uuid,
) -> crate::error::ApiResult<()> {
    if !require_coi(db, tenant_id).await {
        return Ok(());
    }
    let today = chrono::Utc::now().date_naive();
    if coi_current(db, tenant_id, counterparty_id, today).await? {
        return Ok(());
    }
    let reason = override_reason.map(str::trim).filter(|r| !r.is_empty());
    let Some(reason) = reason else {
        return Err(crate::error::ApiError::Conflict(
            "this vendor has no current liability insurance on file; add their certificate, \
             or give a reason to send them anyway"
                .into(),
        ));
    };
    crate::audit::record(
        db,
        actor,
        crate::audit::actions::VENDOR_COI_OVERRIDE,
        Some("maintenance_ticket"),
        Some(ticket_id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "counterparty_id": counterparty_id, "reason": reason })),
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssn_rules() {
        assert_eq!(validate_tin("ssn", "123-45-6789").unwrap(), "123456789");
        for bad in [
            "000-12-3456",
            "666-12-3456",
            "900-12-3456",
            "123-00-4567",
            "123-45-0000",
            "12345678",
            "123-45-678x",
        ] {
            assert!(validate_tin("ssn", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ein_rules() {
        assert_eq!(validate_tin("ein", "12-3456789").unwrap(), "123456789");
        assert!(
            validate_tin("ein", "07-1234567").is_err(),
            "07 is not an IRS prefix"
        );
        assert!(validate_tin("ein", "89-1234567").is_err());
        assert!(validate_tin("vat", "12-3456789").is_err());
    }

    #[test]
    fn shapes() {
        assert_eq!(format_tin("ssn", "123456789"), "123-45-6789");
        assert_eq!(format_tin("ein", "123456789"), "12-3456789");
        assert_eq!(mask_tin("ssn", "6789"), "•••-••-6789");
        assert_eq!(mask_tin("ein", "6789"), "••-•••6789");
    }

    #[test]
    fn insurance_states() {
        let t = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        assert_eq!(insurance_state("2026-09-30", t), "expired");
        assert_eq!(insurance_state("2026-10-01", t), "expiring");
        assert_eq!(insurance_state("2026-10-31", t), "expiring");
        assert_eq!(insurance_state("2026-11-01", t), "current");
        assert_eq!(insurance_state("soon", t), "expired");
    }
}
