//! Accounting invariants for the multi-entity ledger.
//!
//! The GL is partitioned by `entity_id`: each legal entity (LLC) keeps its own
//! books, and **trust** (escrow) accounts carry the *no-commingling* invariant —
//! a posting may never move funds between two different entities' trust ledgers
//! (§11.2). This is a domain rule, enforced here rather than only in the UI, so
//! every code path that records a trust movement goes through one check.
//!
//! A full double-entry posting engine is out of scope for this milestone; the
//! ledger tables land with the accounting module. What ships now is the
//! invariant itself, as the single guard that future postings must call — so the
//! rule cannot be quietly violated when that engine arrives.

use crate::error::ApiError;
use uuid::Uuid;

/// One side of a posting against a bank account.
#[derive(Clone, Copy, Debug)]
pub struct PostingLeg {
    /// The legal entity (LLC) whose ledger this leg belongs to.
    pub entity_id: Uuid,
    /// Whether the account is a `trust`/escrow account (vs `operating`).
    pub is_trust: bool,
}

/// Assert a transfer does not commingle two entities' trust funds.
///
/// A transfer that touches a **trust** account on each side is only legal when
/// both sides belong to the **same** legal entity. Operating-to-operating and
/// operating-to-trust transfers across entities are permitted (e.g. a management
/// fee sweep); trust-to-trust across entities is the commingling the rule forbids.
pub fn assert_no_commingling(from: PostingLeg, to: PostingLeg) -> Result<(), ApiError> {
    if from.is_trust && to.is_trust && from.entity_id != to.entity_id {
        return Err(ApiError::BadRequest(
            "commingling violation: a trust posting may not move funds between two \
             entities' trust ledgers"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(entity: u128, trust: bool) -> PostingLeg {
        PostingLeg {
            entity_id: Uuid::from_u128(entity),
            is_trust: trust,
        }
    }

    #[test]
    fn same_entity_trust_transfer_ok() {
        assert!(assert_no_commingling(leg(1, true), leg(1, true)).is_ok());
    }

    #[test]
    fn cross_entity_trust_transfer_rejected() {
        assert!(assert_no_commingling(leg(1, true), leg(2, true)).is_err());
    }

    #[test]
    fn cross_entity_operating_transfer_ok() {
        assert!(assert_no_commingling(leg(1, false), leg(2, false)).is_ok());
        // Operating -> trust across entities (e.g. funding an escrow) is allowed.
        assert!(assert_no_commingling(leg(1, false), leg(2, true)).is_ok());
    }
}
