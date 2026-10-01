//! **Step processes** (Vantedge roadmap areas 3 and 7): a reusable engine for
//! work that happens in an order — a unit turnover, a new house's onboarding,
//! a campsite turn.
//!
//! * `process_template` / `process_template_step` — the recipe: ordered steps,
//!   each with an owner role, the steps it waits on, a due offset, whether it
//!   is required, whether it needs a photo, and whether it opens a work order.
//! * `process` — one run of a recipe against a property (and a unit).
//! * `process_step` — that run's steps, each `blocked → ready → doing →
//!   done / skipped`. A step becomes ready when what it waits on is finished.
//!
//! One active run per kind per unit (or per property, when there is no unit).
//! All four tables are tenant-owned with enforced row-level security.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS process_template (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    kind text NOT NULL,
    name text NOT NULL,
    is_default boolean NOT NULL DEFAULT false,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_process_template_tenant_kind ON process_template (tenant_id, kind);

CREATE TABLE IF NOT EXISTS process_template_step (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    template_id uuid NOT NULL,
    position integer NOT NULL,
    key text NOT NULL,
    title text NOT NULL,
    description text NULL,
    owner_role text NOT NULL DEFAULT 'office',
    depends_on jsonb NOT NULL DEFAULT '[]'::jsonb,
    due_offset_days integer NOT NULL DEFAULT 0,
    required boolean NOT NULL DEFAULT true,
    requires_photo boolean NOT NULL DEFAULT false,
    ticket_category text NULL,
    ticket_priority text NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_process_template_step_key ON process_template_step (template_id, key);

CREATE TABLE IF NOT EXISTS process (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    kind text NOT NULL,
    property_id uuid NOT NULL,
    unit_id uuid NULL,
    lease_id uuid NULL,
    template_id uuid NULL,
    title text NOT NULL,
    status text NOT NULL DEFAULT 'active',
    started_on text NOT NULL,
    target_date text NULL,
    finished_on text NULL,
    override_reason text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_process_tenant_status ON process (tenant_id, status);
CREATE INDEX IF NOT EXISTS idx_process_property ON process (property_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_process_active_unit
    ON process (kind, unit_id) WHERE status = 'active' AND unit_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_process_active_property
    ON process (kind, property_id) WHERE status = 'active' AND unit_id IS NULL;

CREATE TABLE IF NOT EXISTS process_step (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    process_id uuid NOT NULL,
    position integer NOT NULL,
    key text NOT NULL,
    title text NOT NULL,
    description text NULL,
    owner_role text NOT NULL DEFAULT 'office',
    assignee_user_id uuid NULL,
    depends_on jsonb NOT NULL DEFAULT '[]'::jsonb,
    due_on text NULL,
    required boolean NOT NULL DEFAULT true,
    requires_photo boolean NOT NULL DEFAULT false,
    status text NOT NULL DEFAULT 'blocked',
    started_at timestamptz NULL,
    done_at timestamptz NULL,
    done_by uuid NULL,
    skip_reason text NULL,
    note text NULL,
    ticket_id uuid NULL,
    cost_cents bigint NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_process_step_key ON process_step (process_id, key);
CREATE INDEX IF NOT EXISTS idx_process_step_process ON process_step (process_id);
CREATE INDEX IF NOT EXISTS idx_process_step_ticket ON process_step (ticket_id) WHERE ticket_id IS NOT NULL;
"#;

/// The isolation predicate, empty-GUC safe (see migration `000038`).
const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const TABLES: &[&str] = &[
    "process_step",
    "process",
    "process_template_step",
    "process_template",
];

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
