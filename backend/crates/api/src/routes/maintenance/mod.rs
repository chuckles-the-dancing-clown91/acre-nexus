//! Maintenance work orders — tenant-scoped tickets against properties (and
//! optionally units/leases), with an activity timeline of comments and status
//! changes, and USD cost labels for display.

pub mod add_comment;
pub mod create_ticket;
pub mod dto;
pub mod get_ticket;
pub mod list_property_tickets;
pub mod list_tickets;
pub mod plans;
pub mod portal;
pub mod property_maintenance;
pub mod quotes;
pub mod update_ticket;

/// Ticket statuses that count as still-open work (everything before the ticket
/// is resolved/closed). Used to split the maintenance tab into "open" vs
/// "history".
pub const OPEN_STATUSES: &[&str] = &["open", "triage", "scheduled", "in_progress", "on_hold"];

/// Whether a ticket status is still open work.
pub fn is_open(status: &str) -> bool {
    OPEN_STATUSES.contains(&status)
}
