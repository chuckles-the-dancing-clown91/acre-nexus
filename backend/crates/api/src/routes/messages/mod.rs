//! Resident ↔ manager messaging (roadmap Phase 5, issue #9) — "message the
//! manager" from the renter portal, answered from the console.
//!
//! One [`entity::message_thread`] per conversation on a lease, a flat
//! [`entity::message`] timeline underneath. Residents use the unguarded
//! `/my/messages` routes (scoped to their own lease, like every `/my/*`
//! surface); staff read with `message:read` and reply/close with
//! `message:manage`. Both directions notify through the Phase 1 substrate.

pub mod console;
pub mod dto;
pub mod portal;

use crate::error::ApiResult;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use uuid::Uuid;

/// Longest accepted subject / message body.
pub const MAX_SUBJECT_CHARS: usize = 200;
pub const MAX_BODY_CHARS: usize = 5_000;

/// Validate + normalize a message body (pure).
pub fn clean_body(raw: &str) -> Result<String, String> {
    let body = raw.trim();
    if body.is_empty() {
        return Err("message body is required".into());
    }
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(format!("message body exceeds {MAX_BODY_CHARS} characters"));
    }
    Ok(body.to_string())
}

/// Validate + normalize a thread subject (pure).
pub fn clean_subject(raw: &str) -> Result<String, String> {
    let subject = raw.trim();
    if subject.is_empty() {
        return Err("subject is required".into());
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS {
        return Err(format!("subject exceeds {MAX_SUBJECT_CHARS} characters"));
    }
    Ok(subject.to_string())
}

/// A short body excerpt for notification previews (pure).
pub fn preview(body: &str) -> String {
    const MAX: usize = 140;
    if body.chars().count() <= MAX {
        body.to_string()
    } else {
        let cut: String = body.chars().take(MAX).collect();
        format!("{cut}…")
    }
}

/// Append one message to a thread and bump its `last_message_at`.
#[allow(clippy::too_many_arguments)]
pub async fn append_message(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    thread: &entity::message_thread::Model,
    sender_user_id: Uuid,
    sender_kind: &str,
    sender_name: &str,
    body: String,
) -> ApiResult<entity::message::Model> {
    let now = chrono::Utc::now();
    let saved = entity::message::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread.id),
        sender_user_id: Set(sender_user_id),
        sender_kind: Set(sender_kind.to_string()),
        sender_name: Set(sender_name.to_string()),
        body: Set(body),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    let mut am: entity::message_thread::ActiveModel = thread.clone().into();
    am.last_message_at = Set(now.into());
    // A resident writing into a closed thread reopens it; staff replies keep
    // the thread's state.
    if thread.status == "closed" && sender_kind == "resident" {
        am.status = Set("open".into());
    }
    am.update(db).await?;

    crate::audit::record(
        db,
        Some(sender_user_id),
        crate::audit::actions::MESSAGE_SEND,
        Some("message_thread"),
        Some(thread.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "sender_kind": sender_kind })),
    )
    .await;

    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_trimmed_and_bounded() {
        assert_eq!(clean_body("  hello  ").unwrap(), "hello");
        assert!(clean_body("   ").is_err());
        assert!(clean_body(&"x".repeat(MAX_BODY_CHARS + 1)).is_err());
        assert!(clean_body(&"x".repeat(MAX_BODY_CHARS)).is_ok());
    }

    #[test]
    fn subjects_are_trimmed_and_bounded() {
        assert_eq!(clean_subject(" Leaky tap ").unwrap(), "Leaky tap");
        assert!(clean_subject("").is_err());
        assert!(clean_subject(&"s".repeat(MAX_SUBJECT_CHARS + 1)).is_err());
    }

    #[test]
    fn previews_truncate_long_bodies() {
        assert_eq!(preview("short"), "short");
        let long = "y".repeat(300);
        let p = preview(&long);
        assert!(p.chars().count() <= 141);
        assert!(p.ends_with('…'));
    }
}
