//! **Moving in and out, and advertising**: imports from other property
//! management tools (each run kept, with what it created so it can be undone),
//! and listing syndication to the rental portals (one feed per channel, behind
//! a secret URL, with a record of every pull).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS import_batch (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    kind text NOT NULL,
    source text NOT NULL DEFAULT 'generic',
    filename text NOT NULL DEFAULT '',
    status text NOT NULL DEFAULT 'draft',
    content text NULL,
    mapping jsonb NOT NULL DEFAULT '{}'::jsonb,
    row_count integer NOT NULL DEFAULT 0,
    summary jsonb NOT NULL DEFAULT '{}'::jsonb,
    created jsonb NOT NULL DEFAULT '[]'::jsonb,
    errors jsonb NOT NULL DEFAULT '[]'::jsonb,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    committed_at timestamptz NULL,
    undone_at timestamptz NULL,
    undone_by uuid NULL
);
CREATE INDEX IF NOT EXISTS idx_import_batch_tenant ON import_batch (tenant_id, created_at DESC);

ALTER TABLE listing ADD COLUMN IF NOT EXISTS state text NOT NULL DEFAULT '';
ALTER TABLE listing ADD COLUMN IF NOT EXISTS postal_code text NOT NULL DEFAULT '';
ALTER TABLE listing ADD COLUMN IF NOT EXISTS syndicate boolean NOT NULL DEFAULT true;
-- Listings made before this carry their property's state and ZIP.
UPDATE listing SET state = p.state, postal_code = p.postal_code
  FROM property p
 WHERE listing.property_id = p.id AND listing.state = '' AND listing.postal_code = '';

CREATE TABLE IF NOT EXISTS syndication_channel (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    channel text NOT NULL,
    enabled boolean NOT NULL DEFAULT false,
    feed_token text NOT NULL UNIQUE,
    contact_name text NULL,
    contact_email text NULL,
    contact_phone text NULL,
    last_pulled_at timestamptz NULL,
    last_pull_agent text NULL,
    pull_count integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, channel)
);

CREATE TABLE IF NOT EXISTS syndication_pull (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    channel text NOT NULL,
    user_agent text NULL,
    listings integer NOT NULL DEFAULT 0,
    pulled_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_syndication_pull ON syndication_pull (tenant_id, channel, pulled_at DESC);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        for t in ["import_batch", "syndication_channel", "syndication_pull"] {
            let policy = format!("{t}_tenant_isolation");
            db.execute_unprepared(&format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS {policy} ON {t}; \
                 CREATE POLICY {policy} ON {t} \
                   USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE IF EXISTS syndication_pull; \
                 DROP TABLE IF EXISTS syndication_channel; \
                 ALTER TABLE listing DROP COLUMN IF EXISTS syndicate; \
                 ALTER TABLE listing DROP COLUMN IF EXISTS postal_code; \
                 ALTER TABLE listing DROP COLUMN IF EXISTS state; \
                 DROP TABLE IF EXISTS import_batch;",
            )
            .await?;
        Ok(())
    }
}
