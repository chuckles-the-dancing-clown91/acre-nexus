//! Convenient re-exports of every entity type, sourced from the three
//! per-domain crates (`acre_user` / `acre_property` / `acre_client`).

pub use acre_client::entity::application::Entity as Application;
pub use acre_client::entity::counterparty::Entity as Counterparty;
pub use acre_client::entity::counterparty_note::Entity as CounterpartyNote;

pub use acre_user::entity::api_token::Entity as ApiToken;
pub use acre_user::entity::audit_log::Entity as AuditLog;
pub use acre_user::entity::background_job::Entity as BackgroundJob;
pub use acre_user::entity::membership::Entity as Membership;
pub use acre_user::entity::permission::Entity as Permission;
pub use acre_user::entity::profile_type::Entity as ProfileType;
pub use acre_user::entity::refresh_token::Entity as RefreshToken;
pub use acre_user::entity::role::Entity as Role;
pub use acre_user::entity::role_permission::Entity as RolePermission;
pub use acre_user::entity::tenant::Entity as Tenant;
pub use acre_user::entity::tenant_module::Entity as TenantModule;
pub use acre_user::entity::theme::Entity as Theme;
pub use acre_user::entity::user::Entity as User;
pub use acre_user::entity::user_profile::Entity as UserProfile;
pub use acre_user::entity::user_role::Entity as UserRole;

pub use acre_property::entity::enrichment_run::Entity as EnrichmentRun;
pub use acre_property::entity::lease::Entity as Lease;
pub use acre_property::entity::lease_payment::Entity as LeasePayment;
pub use acre_property::entity::lien::Entity as Lien;
pub use acre_property::entity::listing::Entity as Listing;
pub use acre_property::entity::llc::Entity as Llc;
pub use acre_property::entity::maintenance_ticket::Entity as MaintenanceTicket;
pub use acre_property::entity::mortgage::Entity as Mortgage;
pub use acre_property::entity::ownership::Entity as Ownership;
pub use acre_property::entity::property::Entity as Property;
pub use acre_property::entity::property_detail::Entity as PropertyDetail;
pub use acre_property::entity::property_school::Entity as PropertySchool;
pub use acre_property::entity::property_tax::Entity as PropertyTax;
pub use acre_property::entity::property_utility::Entity as PropertyUtility;
pub use acre_property::entity::property_valuation::Entity as PropertyValuation;
pub use acre_property::entity::ticket_comment::Entity as TicketComment;
pub use acre_property::entity::unit::Entity as Unit;
pub use acre_property::entity::workflow_event::Entity as WorkflowEvent;
