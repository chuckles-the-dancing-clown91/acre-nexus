//! **The full property profile**: permits pulled on the property, its
//! insurance policies, the schools it's zoned for (a school row can now say
//! the property is in its attendance zone, and a team's edits survive a data
//! refresh), and action items: to-dos that hang off the property or anything
//! on it (a permit to close out, a policy to renew, a school zone to confirm).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS property_permit (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES property(id) ON DELETE CASCADE,
    unit_id uuid NULL REFERENCES unit(id) ON DELETE SET NULL,
    permit_number text NULL,
    kind text NOT NULL DEFAULT 'building',
    description text NOT NULL,
    status text NOT NULL DEFAULT 'applied',
    jurisdiction text NULL,
    applied_on text NULL,
    issued_on text NULL,
    expires_on text NULL,
    inspection_on text NULL,
    finaled_on text NULL,
    contractor_entity_id uuid NULL,
    contractor_name text NULL,
    valuation_cents bigint NULL,
    fee_cents bigint NULL,
    ticket_id uuid NULL REFERENCES maintenance_ticket(id) ON DELETE SET NULL,
    document_ids jsonb NOT NULL DEFAULT '[]'::jsonb,
    notes text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_property_permit ON property_permit (tenant_id, property_id);

CREATE TABLE IF NOT EXISTS insurance_policy (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES property(id) ON DELETE CASCADE,
    kind text NOT NULL DEFAULT 'property',
    carrier text NOT NULL,
    policy_number text NULL,
    status text NOT NULL DEFAULT 'active',
    effective_on text NULL,
    expires_on text NULL,
    premium_cents bigint NULL,
    coverage_cents bigint NULL,
    deductible_cents bigint NULL,
    agent_name text NULL,
    agent_phone text NULL,
    agent_email text NULL,
    document_ids jsonb NOT NULL DEFAULT '[]'::jsonb,
    notes text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_insurance_policy ON insurance_policy (tenant_id, property_id);

CREATE TABLE IF NOT EXISTS action_item (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    property_id uuid NOT NULL REFERENCES property(id) ON DELETE CASCADE,
    subject_type text NOT NULL DEFAULT 'property',
    subject_id uuid NULL,
    title text NOT NULL,
    notes text NULL,
    due_on text NULL,
    priority text NOT NULL DEFAULT 'normal',
    status text NOT NULL DEFAULT 'open',
    assignee_user_id uuid NULL,
    suggestion_key text NULL,
    created_by uuid NULL,
    completed_at timestamptz NULL,
    completed_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_action_item ON action_item (tenant_id, property_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS uq_action_item_suggestion
    ON action_item (property_id, suggestion_key) WHERE suggestion_key IS NOT NULL;

ALTER TABLE property_school ADD COLUMN IF NOT EXISTS assigned boolean NOT NULL DEFAULT false;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS zone_name text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS zone_verified_on text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS address text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS phone text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS website text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS enrollment integer NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS notes text NULL;
ALTER TABLE property_school ADD COLUMN IF NOT EXISTS updated_at timestamptz NULL;
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        for t in ["property_permit", "insurance_policy", "action_item"] {
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
        let mut sql = String::from(
            "DROP TABLE IF EXISTS action_item; \
             DROP TABLE IF EXISTS insurance_policy; \
             DROP TABLE IF EXISTS property_permit;",
        );
        for c in [
            "assigned",
            "zone_name",
            "zone_verified_on",
            "address",
            "phone",
            "website",
            "enrollment",
            "notes",
            "updated_at",
        ] {
            sql.push_str(&format!(
                " ALTER TABLE property_school DROP COLUMN IF EXISTS {c};"
            ));
        }
        manager.get_connection().execute_unprepared(&sql).await?;
        Ok(())
    }
}
