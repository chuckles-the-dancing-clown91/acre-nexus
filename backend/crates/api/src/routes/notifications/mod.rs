//! User-facing notification routes: the in-app inbox (list, unread count,
//! mark read) and Web Push subscription management. Mounted by the
//! `integrations` module. Inbox rows are always scoped to the signed-in user —
//! no extra permission is required to read your own notifications.

pub mod dto;
pub mod inbox;
pub mod push;
