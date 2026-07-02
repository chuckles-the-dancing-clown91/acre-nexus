//! Integrations settings routes: credential storage (write-only), the outbound
//! notification log, and the inbound webhook ingestion endpoint. Mounted by the
//! `integrations` module (`crate::modules::integrations`).

pub mod delete_secret;
pub mod dto;
pub mod list_notifications;
pub mod list_secrets;
pub mod set_secret;
pub mod webhook;
