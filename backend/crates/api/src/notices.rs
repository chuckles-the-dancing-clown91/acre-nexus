//! The **notice log**: an automatic notice claims its key before it is sent,
//! so a job that runs every few hours sends each notice once.

use chrono::Utc;
use entity::prelude::NoticeLog;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Claim `key` for this workspace. `true` when this call claimed it (send the
/// notice); `false` when it was claimed before (skip).
///
/// Claims through `ON CONFLICT DO NOTHING`, so a duplicate never raises an
/// error that would abort the surrounding transaction.
pub async fn claim(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::sea_query::OnConflict;
    let am = entity::notice_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        key: Set(key.to_string()),
        sent_at: Set(Utc::now().into()),
    };
    let res = NoticeLog::insert(am)
        .on_conflict(
            OnConflict::columns([
                entity::notice_log::Column::TenantId,
                entity::notice_log::Column::Key,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
    Ok(res == 1)
}

/// Whether `key` was already claimed.
#[allow(dead_code)]
pub async fn claimed(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    key: &str,
) -> Result<bool, sea_orm::DbErr> {
    Ok(NoticeLog::find()
        .filter(entity::notice_log::Column::TenantId.eq(tenant_id))
        .filter(entity::notice_log::Column::Key.eq(key))
        .one(db)
        .await?
        .is_some())
}
