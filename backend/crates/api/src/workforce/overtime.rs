//! **Overtime** — which hours of a workweek are paid at 1×, 1.5× and 2×.
//!
//! Ported rule-for-rule from Alpha Power Wash, working in whole minutes so
//! nothing rounds on the way:
//!
//! * **Weekly (FLSA)** — anything over 40 hours in a Monday–Sunday workweek
//!   is paid at 1.5×.
//! * **California daily** (Labor Code §510) — in addition: over 8 hours in a
//!   day is 1.5×, over 12 is 2×; on the seventh consecutive day worked in the
//!   workweek the first 8 hours are 1.5× and the rest 2×. Daily overtime hours
//!   do not count again toward the weekly 40.
//!
//! A time entry belongs to the local day it *started* on (a shift past midnight
//! isn't split), and a person who isn't overtime-eligible (1099 contractors) is
//! paid straight time for everything.

use chrono::{Datelike, Duration, NaiveDate};
use std::collections::BTreeMap;

const WEEKLY_LIMIT: i64 = 40 * 60;
const DAILY_OT_AFTER: i64 = 8 * 60;
const DAILY_DT_AFTER: i64 = 12 * 60;

/// The overtime rule a workspace pays by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Weekly,
    California,
}

impl Rule {
    pub fn parse(s: &str) -> Rule {
        match s.trim().to_lowercase().as_str() {
            "california" | "ca" | "california_daily" => Rule::California,
            _ => Rule::Weekly,
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Rule::California => {
                "California daily overtime: 1.5× over 8 hours in a day and over 40 \
                 straight-time hours in a Monday–Sunday week; 2× over 12 hours in a day. \
                 On a seventh consecutive day worked the first 8 hours are 1.5× and the \
                 rest 2×."
            }
            Rule::Weekly => {
                "Overtime is 1.5× over 40 hours in a Monday–Sunday week; no daily overtime."
            }
        }
    }
}

/// A week's minutes by pay multiplier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Split {
    pub regular: i64,
    /// Paid at 1.5×.
    pub overtime: i64,
    /// Paid at 2×.
    pub double: i64,
}

impl Split {
    pub fn minutes(&self) -> i64 {
        self.regular + self.overtime + self.double
    }

    /// Gross pay in cents at an hourly rate in cents, rounded half-up once.
    pub fn pay_cents(&self, rate_cents: i64) -> i64 {
        // (reg×2 + ot×3 + dt×4) × rate / (60 × 2), in exact integer math.
        let weighted = self.regular * 2 + self.overtime * 3 + self.double * 4;
        div_round(weighted * rate_cents, 120)
    }

    /// Just the overtime premium over straight time (0.5× on OT, 1× on DT).
    pub fn premium_cents(&self, rate_cents: i64) -> i64 {
        div_round((self.overtime + self.double * 2) * rate_cents, 120)
    }
}

/// Integer division rounding half away from zero.
pub fn div_round(n: i64, d: i64) -> i64 {
    if (n >= 0) == (d >= 0) {
        (n + d / 2) / d
    } else {
        (n - d / 2) / d
    }
}

/// The Monday a date's workweek starts on.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_monday() as i64)
}

/// Allocate one workweek's minutes (by local day) to straight time, overtime
/// and double time.
pub fn split_week(minutes_by_day: &BTreeMap<NaiveDate, i64>, rule: Rule, eligible: bool) -> Split {
    let days: BTreeMap<NaiveDate, i64> = minutes_by_day
        .iter()
        .filter(|(_, m)| **m > 0)
        .map(|(d, m)| (*d, *m))
        .collect();
    let total: i64 = days.values().sum();
    if !eligible || total == 0 {
        return Split {
            regular: total,
            ..Default::default()
        };
    }
    if rule == Rule::Weekly {
        let regular = total.min(WEEKLY_LIMIT);
        return Split {
            regular,
            overtime: total - regular,
            double: 0,
        };
    }

    // The seventh-day premium applies only when every day of the week was worked.
    let first = *days.keys().next().expect("non-empty");
    let monday = week_start(first);
    let seventh = monday + Duration::days(6);
    let all_seven = (0..7).all(|i| days.contains_key(&(monday + Duration::days(i))));

    let mut s = Split::default();
    for (day, &m) in &days {
        let (mut reg, mut ot, dt) = if all_seven && *day == seventh {
            (0, m.min(DAILY_OT_AFTER), (m - DAILY_OT_AFTER).max(0))
        } else {
            (
                m.min(DAILY_OT_AFTER),
                (m - DAILY_OT_AFTER).clamp(0, DAILY_DT_AFTER - DAILY_OT_AFTER),
                (m - DAILY_DT_AFTER).max(0),
            )
        };
        // Straight-time minutes past 40 in the week become overtime as well.
        if s.regular + reg > WEEKLY_LIMIT {
            let spill = s.regular + reg - WEEKLY_LIMIT;
            reg -= spill;
            ot += spill;
        }
        s.regular += reg;
        s.overtime += ot;
        s.double += dt;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 60;

    /// Hours for Mon..Sun of the week of 2026-09-14; zero = day off.
    fn week(hours: &[i64]) -> BTreeMap<NaiveDate, i64> {
        let mon = NaiveDate::from_ymd_opt(2026, 9, 14).unwrap();
        hours
            .iter()
            .enumerate()
            .filter(|(_, h)| **h > 0)
            .map(|(i, h)| (mon + Duration::days(i as i64), h * H))
            .collect()
    }

    fn hrs(s: Split) -> (i64, i64, i64) {
        (s.regular / H, s.overtime / H, s.double / H)
    }

    #[test]
    fn weekly_rule_is_forty_then_time_and_a_half() {
        let s = split_week(&week(&[9, 9, 9, 9, 9]), Rule::Weekly, true);
        assert_eq!(hrs(s), (40, 5, 0));
        assert_eq!(s.pay_cents(2000), 95_000); // 40×20 + 5×30
    }

    #[test]
    fn weekly_rule_ignores_long_days() {
        assert_eq!(
            hrs(split_week(&week(&[13]), Rule::Weekly, true)),
            (13, 0, 0)
        );
    }

    #[test]
    fn california_daily_thresholds() {
        let s = split_week(&week(&[13]), Rule::California, true);
        assert_eq!(hrs(s), (8, 4, 1));
        assert_eq!(s.pay_cents(2000), 32_000); // 8×20 + 4×30 + 1×40
    }

    #[test]
    fn california_daily_overtime_does_not_double_count_toward_forty() {
        let s = split_week(&week(&[10, 10, 10, 10, 10]), Rule::California, true);
        assert_eq!(hrs(s), (40, 10, 0));
    }

    #[test]
    fn california_weekly_spill_on_straight_time() {
        let s = split_week(&week(&[8, 8, 8, 8, 8, 8]), Rule::California, true);
        assert_eq!(hrs(s), (40, 8, 0));
    }

    #[test]
    fn california_seventh_consecutive_day() {
        let s = split_week(&week(&[8, 8, 8, 8, 8, 8, 10]), Rule::California, true);
        assert_eq!(hrs(s), (40, 16, 2));
    }

    #[test]
    fn seventh_day_rule_needs_all_seven_days_worked() {
        let s = split_week(&week(&[8, 8, 0, 8, 8, 8, 10]), Rule::California, true);
        assert_eq!(hrs(s), (40, 10, 0));
    }

    #[test]
    fn not_eligible_is_all_straight_time() {
        let s = split_week(&week(&[13, 13, 13, 13, 13]), Rule::California, false);
        assert_eq!(hrs(s), (65, 0, 0));
    }

    #[test]
    fn empty_week() {
        assert_eq!(
            split_week(&BTreeMap::new(), Rule::California, true).minutes(),
            0
        );
    }

    #[test]
    fn premium_is_the_extra_over_straight_time() {
        let s = split_week(&week(&[13]), Rule::California, true);
        // 4h × $10 extra + 1h × $20 extra = $60.
        assert_eq!(s.premium_cents(2000), 6_000);
        assert_eq!(s.pay_cents(2000) - s.minutes() * 2000 / 60, 6_000);
    }

    #[test]
    fn odd_minutes_round_once() {
        // 7 minutes at $19.99/h = $2.3321… → $2.33
        let s = Split {
            regular: 7,
            ..Default::default()
        };
        assert_eq!(s.pay_cents(1999), 233);
        assert_eq!(div_round(-5, 2), -3);
    }

    #[test]
    fn week_starts_monday() {
        let sun = NaiveDate::from_ymd_opt(2026, 9, 20).unwrap();
        assert_eq!(
            week_start(sun),
            NaiveDate::from_ymd_opt(2026, 9, 14).unwrap()
        );
    }
}
