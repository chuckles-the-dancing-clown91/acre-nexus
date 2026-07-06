//! **Property profile build-out**: the columns the property dossier needs beyond
//! the thin base record.
//!
//! * `property.image_url` — the property's hero photo, shown top-left on the
//!   profile. A plain URL into the object store (or any CDN); the blob itself is
//!   an ordinary [`document`]/asset, this is just the display pointer.
//! * `document.category` — a coarse filing bucket (`insurance` | `loan` |
//!   `title` | `tax` | `lease` | `inspection` | `permit` | `other`) so the
//!   documents tab can group insurance docs, loan docs, etc.
//! * `document.requires_wet_ink` — the original needs a physical ("wet ink")
//!   signature, so a paper copy is the record of truth.
//! * `document.physical_location` — where that wet-ink original is filed (e.g.
//!   "Fireproof safe — HQ, Drawer 3"), so it can actually be found.
//!
//! Both tables already exist and already carry enforced RLS (their `tenant_id`
//! is `NOT NULL`); adding columns needs no policy changes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

async fn add_col(manager: &SchemaManager<'_>, table: &str, mut c: ColumnDef) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .add_column_if_not_exists(&mut c)
                .to_owned(),
        )
        .await
}

async fn drop_col(manager: &SchemaManager<'_>, table: &str, column: &str) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .drop_column(Alias::new(column))
                .to_owned(),
        )
        .await
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- property: hero image ----
        add_col(manager, "property", col("image_url").string().null().take()).await?;

        // ---- document: filing category + wet-ink original tracking ----
        add_col(manager, "document", col("category").string().null().take()).await?;
        add_col(
            manager,
            "document",
            col("requires_wet_ink")
                .boolean()
                .not_null()
                .default(false)
                .take(),
        )
        .await?;
        add_col(
            manager,
            "document",
            col("physical_location").string().null().take(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_col(manager, "document", "physical_location").await?;
        drop_col(manager, "document", "requires_wet_ink").await?;
        drop_col(manager, "document", "category").await?;
        drop_col(manager, "property", "image_url").await?;
        Ok(())
    }
}
