//! **Accounts payable** endpoints (issue #58) — vendor bills through their
//! `draft → submitted → approved → paid` lifecycle. Reading needs
//! `payable:read`; creating/editing/submitting needs `payable:manage`;
//! approving, rejecting, and paying need `payable:approve`.

pub mod approve;
pub mod create;
pub mod dto;
pub mod get;
pub mod list;
pub mod pay;
pub mod reject;
pub mod submit;
pub mod update;
pub mod void;
