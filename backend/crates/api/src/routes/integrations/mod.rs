//! Integrations settings routes: credential storage (write-only), the outbound
//! notification log, and the inbound webhook ingestion endpoint. Mounted by the
//! `integrations` module (`crate::modules::integrations`).

pub mod create_provider;
pub mod delete_provider;
pub mod delete_secret;
pub mod dto;
pub mod list_notifications;
pub mod list_providers;
pub mod list_secrets;
pub mod set_secret;
pub mod templates;
pub mod test_provider;
pub mod update_provider;
pub mod webhook;

/// The vault key a provider's credential lives under.
pub fn provider_secret_ref(id: uuid::Uuid) -> String {
    format!("provider.{id}.credential")
}
