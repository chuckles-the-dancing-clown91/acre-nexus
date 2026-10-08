//! **Password links** — the one-time links that let a person choose a password:
//! an `invite` when the office adds them (7 days) and a `reset` from "Forgot your
//! password?" (24 hours).
//!
//! A link is `{PUBLIC_APP_URL}/set-password?token=…`. Only a SHA-256 hash of the
//! token is stored; issuing a new link of the same purpose retires the older
//! ones, and using a link marks it used, so each works exactly once. Links go out
//! as an email (always) and a text when the person's profile has a phone.

use crate::auth::{hash_secret, random_secret};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use entity::prelude::{Membership, PasswordToken, UserProfile};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde_json::json;
use uuid::Uuid;

pub const PURPOSE_INVITE: &str = "invite";
pub const PURPOSE_RESET: &str = "reset";

/// Shortest password the set/reset endpoints accept.
pub const MIN_PASSWORD_LEN: usize = 10;

/// How long a link of each purpose stays valid.
pub fn lifetime(purpose: &str) -> Duration {
    match purpose {
        PURPOSE_INVITE => Duration::days(7),
        _ => Duration::hours(24),
    }
}

/// Why a password is refused, or `None` when it is acceptable.
pub fn password_problem(password: &str, email: &str) -> Option<String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Some(format!(
            "use at least {MIN_PASSWORD_LEN} characters — a short phrase is easiest to remember"
        ));
    }
    if password.trim().is_empty() {
        return Some("the password can't be only spaces".into());
    }
    let lower = password.to_lowercase();
    if !email.is_empty() && lower == email.to_lowercase() {
        return Some("the password can't be your email address".into());
    }
    if ["password12", "1234567890", "qwertyuiop", "letmein123"].contains(&lower.as_str()) {
        return Some("that password is too common".into());
    }
    None
}

/// Mint a new link for `user_id`, retiring any unused link of the same purpose.
/// Returns the raw token (only ever held in memory and in the message sent).
pub async fn issue(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    purpose: &str,
) -> Result<(String, entity::password_token::Model), DbErr> {
    issue_valid_for(db, user_id, purpose, lifetime(purpose)).await
}

/// [`issue`] with a shorter life than the purpose's own — capped at it, so a
/// caller can only ever narrow a link's window.
pub async fn issue_valid_for(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    purpose: &str,
    valid_for: Duration,
) -> Result<(String, entity::password_token::Model), DbErr> {
    let valid_for = valid_for.min(lifetime(purpose));
    let now = Utc::now();
    // Retire older unused links of this purpose so only the newest one works.
    let open = PasswordToken::find()
        .filter(entity::password_token::Column::UserId.eq(user_id))
        .filter(entity::password_token::Column::Purpose.eq(purpose))
        .filter(entity::password_token::Column::UsedAt.is_null())
        .all(db)
        .await?;
    for t in open {
        let mut am: entity::password_token::ActiveModel = t.into();
        am.used_at = Set(Some(now.into()));
        am.update(db).await?;
    }

    let token = random_secret(32);
    let row = entity::password_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        purpose: Set(purpose.to_string()),
        token_hash: Set(hash_secret(&token)),
        expires_at: Set((now + valid_for).into()),
        used_at: Set(None),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok((token, row))
}

/// Look up a link by its raw token. `Ok(None)` for unknown, used or expired.
pub async fn find_valid(
    db: &impl ConnectionTrait,
    token: &str,
) -> Result<Option<entity::password_token::Model>, DbErr> {
    let row = PasswordToken::find()
        .filter(entity::password_token::Column::TokenHash.eq(hash_secret(token)))
        .one(db)
        .await?;
    Ok(row.filter(|r| is_usable(r.used_at, r.expires_at, Utc::now())))
}

/// A link is usable when it hasn't been used and hasn't expired.
pub fn is_usable(
    used_at: Option<DateTime<FixedOffset>>,
    expires_at: DateTime<FixedOffset>,
    now: DateTime<Utc>,
) -> bool {
    used_at.is_none() && expires_at > now
}

/// The public URL of a link.
pub fn link_url(token: &str) -> String {
    format!(
        "{}/set-password?token={token}",
        crate::oauth::public_app_url().trim_end_matches('/')
    )
}

/// The workspace a user's messages are sent from: their own tenant, else the
/// first workspace they belong to. `None` for platform-only staff.
pub async fn home_tenant(db: &impl ConnectionTrait, user: &entity::user::Model) -> Option<Uuid> {
    if let Some(t) = user.tenant_id {
        return Some(t);
    }
    Membership::find()
        .filter(entity::membership::Column::UserId.eq(user.id))
        .filter(entity::membership::Column::TenantId.is_not_null())
        .order_by_desc(entity::membership::Column::IsPrimary)
        .order_by_asc(entity::membership::Column::CreatedAt)
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.tenant_id)
}

/// Queue the email (and, when the profile has a phone, the text) carrying a
/// link. `tenant_id` picks the workspace's providers and branding; returns
/// `false` when there was no workspace to send from.
pub async fn deliver(
    db: &impl ConnectionTrait,
    tenant_id: Option<Uuid>,
    user: &entity::user::Model,
    token: &str,
    row: &entity::password_token::Model,
) -> bool {
    let Some(tenant_id) = tenant_id else {
        tracing::warn!(
            user_id = %user.id,
            "password link not sent: the account has no workspace to send from"
        );
        return false;
    };
    let template = if row.purpose == PURPOSE_INVITE {
        "account_invite"
    } else {
        "password_reset"
    };
    let first_name = user.name.split_whitespace().next().unwrap_or("there");
    let vars = json!({ "name": first_name, "link": link_url(token) });
    let base = |to: &str| {
        json!({
            "template": template,
            "to": to,
            "vars": vars,
            "owner_type": "password_token",
            "owner_id": row.id.to_string(),
            "trigger": row.purpose,
        })
    };

    if let Err(e) =
        crate::scheduler::enqueue(db, tenant_id, "auto_email", base(&user.email), 0).await
    {
        tracing::error!("failed to enqueue password link email: {e}");
        return false;
    }
    let phone = UserProfile::find()
        .filter(entity::user_profile::Column::UserId.eq(user.id))
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|p| p.phone)
        .filter(|p| !p.trim().is_empty());
    if let Some(phone) = phone {
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", base(&phone), 0).await
        {
            tracing::error!("failed to enqueue password link text: {e}");
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetimes() {
        assert_eq!(lifetime(PURPOSE_INVITE), Duration::days(7));
        assert_eq!(lifetime(PURPOSE_RESET), Duration::hours(24));
    }

    #[test]
    fn password_rules() {
        assert!(password_problem("short", "a@b.co").is_some());
        assert!(password_problem("Password12", "a@b.co").is_some());
        assert!(password_problem("me@example.com", "ME@example.com").is_some());
        assert!(password_problem("          ", "a@b.co").is_some());
        assert!(password_problem("correct horse battery", "a@b.co").is_none());
    }

    #[test]
    fn usability() {
        let now = Utc::now();
        let later: DateTime<FixedOffset> = (now + Duration::hours(1)).into();
        let earlier: DateTime<FixedOffset> = (now - Duration::hours(1)).into();
        assert!(is_usable(None, later, now));
        assert!(!is_usable(None, earlier, now));
        assert!(!is_usable(Some(earlier), later, now));
    }

    #[test]
    fn link_shape() {
        let url = link_url("abc");
        assert!(url.ends_with("/set-password?token=abc"));
    }
}
