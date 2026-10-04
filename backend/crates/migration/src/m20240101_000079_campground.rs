//! **Campground reservations** (roadmap area 8): stays on the sites drawn on a
//! campground's site map, with seasons, add-ons and a deposit rule per map.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS campground_config (
    map_id uuid PRIMARY KEY REFERENCES site_map(id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    booking_open boolean NOT NULL DEFAULT false,
    deposit_pct int NOT NULL DEFAULT 25 CHECK (deposit_pct BETWEEN 0 AND 100),
    check_in_time text NOT NULL DEFAULT '15:00',
    check_out_time text NOT NULL DEFAULT '11:00',
    max_nights int NOT NULL DEFAULT 180 CHECK (max_nights BETWEEN 1 AND 366),
    addons jsonb NOT NULL DEFAULT '[]',
    policies text NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS campground_season (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    map_id uuid NOT NULL REFERENCES site_map(id) ON DELETE CASCADE,
    name text NOT NULL,
    start_md text NOT NULL,
    end_md text NOT NULL,
    adjust_pct int NOT NULL DEFAULT 0 CHECK (adjust_pct BETWEEN -90 AND 500),
    min_nights int NOT NULL DEFAULT 1 CHECK (min_nights BETWEEN 1 AND 60),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS ix_campground_season_map ON campground_season (map_id);
CREATE TABLE IF NOT EXISTS stay (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    map_id uuid NOT NULL REFERENCES site_map(id) ON DELETE CASCADE,
    site_id uuid NOT NULL,
    site_name text NOT NULL,
    guest_name text NOT NULL,
    email text NULL,
    phone text NULL,
    check_in date NOT NULL,
    check_out date NOT NULL,
    guests int NOT NULL DEFAULT 1,
    vehicle text NULL,
    rig_length_ft int NULL,
    addons jsonb NOT NULL DEFAULT '[]',
    total_cents bigint NOT NULL,
    deposit_cents bigint NOT NULL,
    paid_cents bigint NOT NULL DEFAULT 0,
    status text NOT NULL CHECK (status IN ('held', 'confirmed', 'checked_in', 'checked_out', 'cancelled')),
    source text NOT NULL DEFAULT 'staff',
    note text NULL,
    token_hash text NULL,
    checked_in_at timestamptz NULL,
    checked_out_at timestamptz NULL,
    cleaned_at timestamptz NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (check_out > check_in)
);
CREATE INDEX IF NOT EXISTS ix_stay_map_dates ON stay (map_id, check_in, check_out);
CREATE INDEX IF NOT EXISTS ix_stay_site ON stay (site_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_stay_token ON stay (token_hash) WHERE token_hash IS NOT NULL;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let c = manager.get_connection();
        c.execute_unprepared(TABLES).await?;
        for t in ["campground_config", "campground_season", "stay"] {
            c.execute_unprepared(&format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 CREATE POLICY {t}_tenant_isolation ON {t} USING ({POLICY}) WITH CHECK ({POLICY});"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS stay; DROP TABLE IF EXISTS campground_season; DROP TABLE IF EXISTS campground_config;")
            .await?;
        Ok(())
    }
}
