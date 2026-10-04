//! **Property types, units and meters.** A property's type decides how it is
//! rented (a single-family home is one unit; an apartment building has many;
//! a campground has sites). Units gain a floor and notes, and each property
//! or unit can have meters (electric, gas, water …) with readings, and who
//! pays for each — which the lease's utility agreement is built from.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const SQL: &str = r#"
ALTER TABLE unit
    ADD COLUMN IF NOT EXISTS floor int NULL,
    ADD COLUMN IF NOT EXISTS notes text NULL;

CREATE TABLE IF NOT EXISTS meter (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES property(id) ON DELETE CASCADE,
    unit_id uuid NULL REFERENCES unit(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('electric', 'gas', 'water', 'sewer', 'trash', 'internet', 'other')),
    label text NOT NULL,
    meter_number text NULL,
    location text NULL,
    provider text NULL,
    unit_of_measure text NULL,
    paid_by text NOT NULL DEFAULT 'tenant' CHECK (paid_by IN ('tenant', 'landlord', 'shared')),
    billing_note text NULL,
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'retired')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS ix_meter_property ON meter (property_id);
CREATE INDEX IF NOT EXISTS ix_meter_unit ON meter (unit_id);

CREATE TABLE IF NOT EXISTS meter_reading (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    meter_id uuid NOT NULL REFERENCES meter(id) ON DELETE CASCADE,
    read_on date NOT NULL,
    reading double precision NOT NULL CHECK (reading >= 0),
    reason text NOT NULL DEFAULT 'routine' CHECK (reason IN ('routine', 'move_in', 'move_out', 'other')),
    lease_id uuid NULL REFERENCES lease(id) ON DELETE SET NULL,
    note text NULL,
    read_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS ix_meter_reading_meter ON meter_reading (meter_id, read_on);

-- Spellings of the same type collapse to one.
UPDATE property SET property_type = 'multi_family'
    WHERE lower(replace(replace(property_type, '-', ''), '_', '')) IN ('multifamily', 'apartment', 'apartments');
UPDATE property SET property_type = 'single_family'
    WHERE lower(replace(replace(property_type, '-', ''), '_', '')) IN ('singlefamily', 'house', 'sfh');

-- A single-family home, townhome or condo is one unit: give each its unit,
-- and put its lease (and equipment) on it.
INSERT INTO unit (id, tenant_id, property_id, unit_number, status, created_at, updated_at)
SELECT gen_random_uuid(), p.tenant_id, p.id, 'Home',
       CASE WHEN p.occupied_units > 0 THEN 'occupied' ELSE 'vacant' END, now(), now()
FROM property p
WHERE p.property_type IN ('single_family', 'townhome', 'condo')
  AND NOT EXISTS (SELECT 1 FROM unit u WHERE u.property_id = p.id);

UPDATE lease l SET unit_id = u.id
FROM unit u, property p
WHERE l.unit_id IS NULL AND u.property_id = l.property_id AND p.id = l.property_id
  AND p.property_type IN ('single_family', 'townhome', 'condo')
  AND (SELECT count(*) FROM unit x WHERE x.property_id = p.id) = 1;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let c = manager.get_connection();
        c.execute_unprepared(SQL).await?;
        for t in ["meter", "meter_reading"] {
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
            .execute_unprepared(
                "DROP TABLE IF EXISTS meter_reading; DROP TABLE IF EXISTS meter; \
                 ALTER TABLE unit DROP COLUMN IF EXISTS floor, DROP COLUMN IF EXISTS notes;",
            )
            .await?;
        Ok(())
    }
}
