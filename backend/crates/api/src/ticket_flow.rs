//! **Work order status flow.** Each move from one status to another follows a
//! rule and leaves a line in the feed:
//!
//! ```text
//! open → triage → scheduled → in progress → resolved → closed
//!            └──── on hold (waiting on something, with a follow-up) ────┘
//! any open status → cancelled; resolved, closed or cancelled → reopened
//! ```
//!
//! * **Scheduled** needs a date.
//! * **On hold** says what it's waiting on and when to chase it (checked in
//!   `update_ticket`).
//! * **Resolved** needs every task done or skipped, or a reason to leave them
//!   open; running clocks on the work order are stopped.
//! * **Closed** follows resolved.
//! * **Cancelled** and **reopening** need a reason.
//!
//! The rules are pure functions over a few facts, so they're tested apart from
//! the database.

pub const STATUSES: &[&str] = &[
    "open",
    "triage",
    "scheduled",
    "in_progress",
    "on_hold",
    "resolved",
    "closed",
    "cancelled",
];

pub fn label(status: &str) -> &'static str {
    match status {
        "open" => "Open",
        "triage" => "Triage",
        "scheduled" => "Scheduled",
        "in_progress" => "In progress",
        "on_hold" => "On hold",
        "resolved" => "Resolved",
        "closed" => "Closed",
        "cancelled" => "Cancelled",
        _ => "Unknown",
    }
}

fn is_done(status: &str) -> bool {
    matches!(status, "resolved" | "closed" | "cancelled")
}

/// Whether the work order may move from one status to another.
pub fn allowed(from: &str, to: &str) -> bool {
    if from == to || !STATUSES.contains(&to) {
        return false;
    }
    match from {
        // Open work can go anywhere but straight to closed.
        "open" | "triage" | "scheduled" | "in_progress" | "on_hold" => to != "closed",
        "resolved" => matches!(to, "closed" | "open" | "in_progress" | "scheduled"),
        "closed" | "cancelled" => matches!(to, "open" | "in_progress"),
        _ => false,
    }
}

/// What the rules need to know about the work order and the request.
#[derive(Debug, Default, Clone)]
pub struct Facts {
    /// Tasks not done or skipped.
    pub open_tasks: i64,
    pub total_tasks: i64,
    pub done_tasks: i64,
    /// A date the work is set for (the due date, or a booked appointment).
    pub scheduled_for: Option<String>,
    /// What the person moving it wrote.
    pub note: Option<String>,
    /// Their reason for resolving with tasks still open.
    pub open_tasks_reason: Option<String>,
    /// Hours logged on the work order, and money spent.
    pub minutes: i64,
    pub spent_cents: i64,
}

/// What a legal move does.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    /// The line for the feed.
    pub line: String,
    /// Whether the resident sees it.
    pub public: bool,
    /// Stop anyone clocked in on this work order.
    pub stop_clocks: bool,
}

fn hours(minutes: i64) -> String {
    let h = minutes as f64 / 60.0;
    if minutes % 60 == 0 {
        format!("{}h", minutes / 60)
    } else {
        format!("{h:.1}h")
    }
}

fn money(cents: i64) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// Decide a move: refuse it with the reason, or say what it does.
pub fn decide(from: &str, to: &str, f: &Facts) -> Result<Plan, String> {
    if !STATUSES.contains(&to) {
        return Err(format!("unknown status: {to}"));
    }
    if from == to {
        return Err(format!("it's already {}", label(to).to_lowercase()));
    }
    if !allowed(from, to) {
        return Err(format!(
            "a {} work order can't go straight to {}",
            label(from).to_lowercase(),
            label(to).to_lowercase()
        ));
    }
    let note = f.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let with_note = |base: String| match note {
        Some(n) => format!("{base} {n}"),
        None => base,
    };
    // Closing a resolved work order is the normal next step; any other move
    // out of a finished status is a reopening.
    let reopening = is_done(from) && !(from == "resolved" && to == "closed");
    if reopening {
        let n = note.ok_or_else(|| "say why it's being reopened".to_string())?;
        return Ok(Plan {
            line: format!("Reopened as {}: {n}", label(to).to_lowercase()),
            public: true,
            stop_clocks: false,
        });
    }
    match to {
        "triage" => Ok(Plan {
            line: with_note("Looked at and sorted.".into()),
            public: false,
            stop_clocks: false,
        }),
        "scheduled" => {
            let date = f
                .scheduled_for
                .as_deref()
                .filter(|d| !d.trim().is_empty())
                .ok_or_else(|| "pick the date it's scheduled for".to_string())?;
            Ok(Plan {
                line: with_note(format!("Scheduled for {date}.")),
                public: true,
                stop_clocks: false,
            })
        }
        "in_progress" => Ok(Plan {
            line: with_note(if from == "on_hold" {
                "Back on it.".into()
            } else {
                "Work started.".into()
            }),
            public: true,
            stop_clocks: false,
        }),
        "on_hold" => Ok(Plan {
            line: with_note("Put on hold.".into()),
            public: false,
            stop_clocks: true,
        }),
        "resolved" => {
            if f.open_tasks > 0
                && f.open_tasks_reason
                    .as_deref()
                    .is_none_or(|r| r.trim().is_empty())
            {
                return Err(format!(
                    "{} task{} still open. Finish {} or say why they can stay open.",
                    f.open_tasks,
                    if f.open_tasks == 1 { " is" } else { "s are" },
                    if f.open_tasks == 1 { "it" } else { "them" }
                ));
            }
            let mut bits = vec![];
            if f.total_tasks > 0 {
                bits.push(format!("{} of {} tasks done", f.done_tasks, f.total_tasks));
            }
            if f.minutes > 0 {
                bits.push(hours(f.minutes));
            }
            if f.spent_cents > 0 {
                bits.push(format!("{} spent", money(f.spent_cents)));
            }
            let mut line = if bits.is_empty() {
                "Resolved.".to_string()
            } else {
                format!("Resolved: {}.", bits.join(", "))
            };
            if let Some(r) = f
                .open_tasks_reason
                .as_deref()
                .filter(|r| !r.trim().is_empty())
            {
                line.push_str(&format!(" Left open: {}.", r.trim().trim_end_matches('.')));
            }
            Ok(Plan {
                line: with_note(line),
                public: true,
                stop_clocks: true,
            })
        }
        "closed" => Ok(Plan {
            line: with_note("Closed.".into()),
            public: false,
            stop_clocks: true,
        }),
        "cancelled" => {
            let n = note.ok_or_else(|| "say why it's cancelled".to_string())?;
            Ok(Plan {
                line: format!("Cancelled: {n}"),
                public: true,
                stop_clocks: true,
            })
        }
        // "open" from an open status: back to the queue.
        _ => Ok(Plan {
            line: with_note("Back in the queue.".into()),
            public: false,
            stop_clocks: false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts::default()
    }

    #[test]
    fn the_happy_path() {
        for (a, b) in [
            ("open", "triage"),
            ("triage", "scheduled"),
            ("scheduled", "in_progress"),
            ("in_progress", "resolved"),
            ("resolved", "closed"),
        ] {
            assert!(allowed(a, b), "{a} -> {b}");
        }
    }

    #[test]
    fn closing_a_resolved_order_needs_no_reason() {
        assert_eq!(
            decide("resolved", "closed", &facts()).unwrap().line,
            "Closed."
        );
    }

    #[test]
    fn some_moves_are_not_allowed() {
        assert!(!allowed("open", "closed"));
        assert!(!allowed("closed", "scheduled"));
        assert!(!allowed("open", "open"));
        assert!(!allowed("open", "bogus"));
    }

    #[test]
    fn scheduling_needs_a_date() {
        let err = decide("triage", "scheduled", &facts()).unwrap_err();
        assert!(err.contains("date"));
        let f = Facts {
            scheduled_for: Some("2026-10-06".into()),
            ..facts()
        };
        let p = decide("triage", "scheduled", &f).unwrap();
        assert_eq!(p.line, "Scheduled for 2026-10-06.");
        assert!(p.public);
    }

    #[test]
    fn resolving_needs_tasks_done_or_a_reason() {
        let f = Facts {
            open_tasks: 2,
            total_tasks: 4,
            done_tasks: 2,
            ..facts()
        };
        assert!(decide("in_progress", "resolved", &f)
            .unwrap_err()
            .contains("2 tasks are still open"));
        let f = Facts {
            open_tasks_reason: Some("Tenant will do the paint".into()),
            ..f
        };
        let p = decide("in_progress", "resolved", &f).unwrap();
        assert!(p.line.starts_with("Resolved: 2 of 4 tasks done."));
        assert!(p.line.contains("Left open: Tenant will do the paint."));
        assert!(p.stop_clocks);
    }

    #[test]
    fn the_resolved_line_sums_it_up() {
        let f = Facts {
            total_tasks: 3,
            done_tasks: 3,
            minutes: 210,
            spent_cents: 18_250,
            note: Some("Tenant signed off.".into()),
            ..facts()
        };
        let p = decide("in_progress", "resolved", &f).unwrap();
        assert_eq!(
            p.line,
            "Resolved: 3 of 3 tasks done, 3.5h, $182.50 spent. Tenant signed off."
        );
    }

    #[test]
    fn cancelling_and_reopening_need_a_reason() {
        assert!(decide("open", "cancelled", &facts()).is_err());
        let f = Facts {
            note: Some("Duplicate of #12".into()),
            ..facts()
        };
        assert_eq!(
            decide("open", "cancelled", &f).unwrap().line,
            "Cancelled: Duplicate of #12"
        );
        assert!(decide("resolved", "in_progress", &facts()).is_err());
        let f = Facts {
            note: Some("Leaking again".into()),
            ..facts()
        };
        assert_eq!(
            decide("closed", "in_progress", &f).unwrap().line,
            "Reopened as in progress: Leaking again"
        );
    }

    #[test]
    fn a_note_rides_along() {
        let f = Facts {
            note: Some("Crew of two.".into()),
            ..facts()
        };
        assert_eq!(
            decide("scheduled", "in_progress", &f).unwrap().line,
            "Work started. Crew of two."
        );
        assert_eq!(
            decide("on_hold", "in_progress", &facts()).unwrap().line,
            "Back on it."
        );
    }
}
