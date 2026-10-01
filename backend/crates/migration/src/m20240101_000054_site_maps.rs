//! **Site maps** (roadmap area 1): a property laid out on a map. An apartment
//! complex draws its buildings and units; a campground draws each campsite,
//! roads and amenities. Everything is stored as GeoJSON in WGS84 (lng, lat) —
//! the format MapLibre and Terra Draw read and write — so one table pair covers
//! a satellite view, an uploaded plan image, and a blank grid.
//!
//! * `site_map` — one map of a property: its kind, base layer, view, the plan
//!   image (a document) with its four corner coordinates, and whether it is
//!   published on the public site.
//! * `site_feature` — one drawn thing: a building, unit, campsite, amenity,
//!   road, boundary, parking area, water, or label, with its geometry and
//!   attributes, optionally linked to a unit record.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS site_map (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    property_id uuid NOT NULL,
    name text NOT NULL,
    kind text NOT NULL DEFAULT 'apartment',
    base_layer text NOT NULL DEFAULT 'satellite',
    center_lng double precision NULL,
    center_lat double precision NULL,
    zoom double precision NOT NULL DEFAULT 17,
    plan_document_id uuid NULL,
    plan_corners jsonb NULL,
    published boolean NOT NULL DEFAULT false,
    notes text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_site_map_property ON site_map (tenant_id, property_id);

CREATE TABLE IF NOT EXISTS site_feature (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    map_id uuid NOT NULL,
    kind text NOT NULL,
    name text NULL,
    geometry jsonb NOT NULL,
    unit_id uuid NULL,
    attrs jsonb NOT NULL DEFAULT '{}'::jsonb,
    position integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_site_feature_map ON site_feature (map_id);
CREATE INDEX IF NOT EXISTS idx_site_feature_unit ON site_feature (unit_id) WHERE unit_id IS NOT NULL;
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const TABLES: &[&str] = &["site_feature", "site_map"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        for t in TABLES {
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
        let db = manager.get_connection();
        for t in TABLES {
            db.execute_unprepared(&format!("DROP TABLE IF EXISTS {t}"))
                .await?;
        }
        Ok(())
    }
}
