//! **Vendor compliance** (fix plan F11–F12): a vendor's W-9 (the taxpayer id
//! the 1099 needs, encrypted) and their insurance certificates with expiry, so
//! nobody is paid without a TIN on file or sent out without current cover.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS vendor_tax_profile (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    counterparty_id uuid NOT NULL,
    legal_name text NOT NULL,
    business_name text NULL,
    classification text NOT NULL,
    tin_type text NOT NULL,
    tin_ciphertext text NOT NULL,
    tin_nonce text NOT NULL,
    tin_last4 text NOT NULL,
    signed_on text NULL,
    document_id uuid NULL,
    updated_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_vendor_tax_profile ON vendor_tax_profile (tenant_id, counterparty_id);

CREATE TABLE IF NOT EXISTS vendor_insurance (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    counterparty_id uuid NOT NULL,
    kind text NOT NULL,
    carrier text NOT NULL,
    policy_number text NULL,
    limit_cents bigint NULL,
    expires_on text NOT NULL,
    document_id uuid NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_vendor_insurance_vendor ON vendor_insurance (tenant_id, counterparty_id);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const TABLES: &[&str] = &["vendor_tax_profile", "vendor_insurance"];

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
