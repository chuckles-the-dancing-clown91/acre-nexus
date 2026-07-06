//! **Calendar / reminders** endpoints (issue #54). Reading the schedule needs
//! `calendar:read`; creating, editing, completing, and cancelling reminders
//! needs `calendar:manage`.

pub mod create;
pub mod delete;
pub mod dto;
pub mod list;
pub mod update;
