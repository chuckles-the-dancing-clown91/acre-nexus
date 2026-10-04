//! **Family-plan features** (roadmap area 17): the related-party guard,
//! foundation mode (income certifications, housing vouchers, the at-cost
//! management fee) and raw land on deals.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const SQL: &str = r#"
ALTER TABLE counterparty
    ADD COLUMN IF NOT EXISTS related_llc_id uuid NULL REFERENCES llc(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS related_owner_id uuid NULL REFERENCES owner(id) ON DELETE SET NULL;

ALTER TABLE llc
    ADD COLUMN IF NOT EXISTS foundation boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS fee_basis text NOT NULL DEFAULT 'percent'
        CHECK (fee_basis IN ('percent', 'at_cost'));

ALTER TABLE deal
    ADD COLUMN IF NOT EXISTS acres double precision NULL CHECK (acres IS NULL OR acres > 0),
    ADD COLUMN IF NOT EXISTS zoning text NULL,
    ADD COLUMN IF NOT EXISTS water_access text NULL,
    ADD COLUMN IF NOT EXISTS power_access text NULL,
    ADD COLUMN IF NOT EXISTS road_access text NULL;

CREATE TABLE IF NOT EXISTS related_party_review (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    subject_type text NOT NULL CHECK (subject_type IN ('vendor_bill', 'lease', 'deal', 'other')),
    subject_id uuid NULL,
    entity_id uuid NULL REFERENCES llc(id) ON DELETE SET NULL,
    counterparty_id uuid NULL REFERENCES counterparty(id) ON DELETE SET NULL,
    summary text NOT NULL,
    reason text NOT NULL,
    amount_cents bigint NULL,
    market_cents bigint NULL,
    market_note text NULL,
    parties jsonb NOT NULL DEFAULT '[]',
    status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'approved', 'rejected')),
    decided_by uuid NULL,
    decided_at timestamptz NULL,
    decision_note text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_related_party_subject
    ON related_party_review (tenant_id, subject_type, subject_id) WHERE subject_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS ix_related_party_status ON related_party_review (tenant_id, status);

CREATE TABLE IF NOT EXISTS income_certification (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    lease_id uuid NOT NULL REFERENCES lease(id) ON DELETE CASCADE,
    effective_on date NOT NULL,
    expires_on date NOT NULL,
    household_size int NOT NULL CHECK (household_size BETWEEN 1 AND 12),
    annual_income_cents bigint NOT NULL CHECK (annual_income_cents >= 0),
    ami_cents bigint NOT NULL CHECK (ami_cents > 0),
    limit_pct int NOT NULL CHECK (limit_pct BETWEEN 1 AND 150),
    qualified boolean NOT NULL,
    notes text NULL,
    certified_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (expires_on > effective_on)
);
CREATE INDEX IF NOT EXISTS ix_income_cert_lease ON income_certification (lease_id, effective_on);

CREATE TABLE IF NOT EXISTS housing_voucher (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    lease_id uuid NOT NULL UNIQUE REFERENCES lease(id) ON DELETE CASCADE,
    authority text NOT NULL,
    contract_number text NULL,
    hap_cents bigint NOT NULL CHECK (hap_cents >= 0),
    starts_on date NOT NULL,
    ends_on date NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let c = manager.get_connection();
        c.execute_unprepared(SQL).await?;
        for t in [
            "related_party_review",
            "income_certification",
            "housing_voucher",
        ] {
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
                "DROP TABLE IF EXISTS housing_voucher; \
                 DROP TABLE IF EXISTS income_certification; \
                 DROP TABLE IF EXISTS related_party_review; \
                 ALTER TABLE deal DROP COLUMN IF EXISTS acres, DROP COLUMN IF EXISTS zoning, \
                   DROP COLUMN IF EXISTS water_access, DROP COLUMN IF EXISTS power_access, \
                   DROP COLUMN IF EXISTS road_access; \
                 ALTER TABLE llc DROP COLUMN IF EXISTS foundation, DROP COLUMN IF EXISTS fee_basis; \
                 ALTER TABLE counterparty DROP COLUMN IF EXISTS related_llc_id, \
                   DROP COLUMN IF EXISTS related_owner_id;",
            )
            .await?;
        Ok(())
    }
}
