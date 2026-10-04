//! **Campground pricing and availability** (roadmap area 8), pure so it is
//! tested without a database.
//!
//! A site's base rates come from its attributes on the map (nightly, weekly,
//! monthly). A stay is priced at the cheapest mix of months (30 nights),
//! weeks (7) and nights the site offers, then adjusted by the average of the
//! season adjustments over its nights; the longest minimum stay among the
//! seasons it touches applies. Add-ons are per stay or per night.

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rates {
    pub night: Option<i64>,
    pub week: Option<i64>,
    pub month: Option<i64>,
}

impl Rates {
    pub fn from_attrs(a: &serde_json::Value) -> Rates {
        let c = |k: &str| a[k].as_f64().map(|v| v as i64).filter(|v| *v > 0);
        Rates {
            night: c("rate_cents_night"),
            week: c("rate_cents_week"),
            month: c("rate_cents_month"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Season {
    pub name: String,
    /// `MM-DD`
    pub start: String,
    /// `MM-DD`, inclusive
    pub end: String,
    pub adjust_pct: i32,
    pub min_nights: i32,
}

impl Season {
    /// Does the season cover this date? A season may wrap the new year
    /// (`11-15` to `02-28`).
    pub fn covers(&self, d: NaiveDate) -> bool {
        let md = format!("{:02}-{:02}", d.month(), d.day());
        if self.start <= self.end {
            md >= self.start && md <= self.end
        } else {
            md >= self.start || md <= self.end
        }
    }
}

pub fn valid_md(s: &str) -> bool {
    NaiveDate::parse_from_str(&format!("2024-{s}"), "%Y-%m-%d").is_ok() && s.len() == 5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct Addon {
    pub key: String,
    pub label: String,
    pub price_cents: i64,
    /// `stay` | `night`
    pub per: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct AddonLine {
    pub key: String,
    pub label: String,
    pub qty: i32,
    pub cents: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, schemars::JsonSchema)]
pub struct Quote {
    pub nights: i64,
    /// Months, weeks and nights the base was charged at.
    pub months: i64,
    pub weeks: i64,
    pub single_nights: i64,
    pub base_cents: i64,
    /// Average season adjustment over the nights, in percent.
    pub season_pct: i64,
    pub seasons: Vec<String>,
    pub stay_cents: i64,
    pub addons: Vec<AddonLine>,
    pub total_cents: i64,
    pub deposit_cents: i64,
    pub min_nights: i64,
}

/// The cheapest base for `n` nights from the rates the site offers:
/// `(cents, months, weeks, nights)`.
pub fn base_for(n: i64, r: &Rates) -> Option<(i64, i64, i64, i64)> {
    let night = r.night?;
    let (week, month) = (r.week, r.month);
    let mut cands: Vec<(i64, i64, i64, i64)> = Vec::new();
    let max_m = if month.is_some() { n / 30 + 1 } else { 0 };
    for m in 0..=max_m {
        let left = n - m * 30;
        let mcost = m * month.unwrap_or(0);
        if left <= 0 {
            cands.push((mcost, m, 0, 0));
            continue;
        }
        let max_w = if week.is_some() { left / 7 + 1 } else { 0 };
        for w in 0..=max_w {
            let rest = (left - w * 7).max(0);
            cands.push((mcost + w * week.unwrap_or(0) + rest * night, m, w, rest));
        }
    }
    // Cheapest; on a tie, the fewest whole units (no padding past the stay).
    cands
        .into_iter()
        .min_by_key(|c| (c.0, c.1 * 30 + c.2 * 7 + c.3))
}

pub enum QuoteError {
    Dates(String),
    NoRate,
    TooShort(i64),
    TooLong(i64),
}

impl QuoteError {
    pub fn message(&self) -> String {
        match self {
            QuoteError::Dates(m) => m.clone(),
            QuoteError::NoRate => "this site has no nightly rate set".into(),
            QuoteError::TooShort(n) => format!("these dates need at least {n} nights"),
            QuoteError::TooLong(n) => format!("stays are limited to {n} nights"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn quote(
    check_in: NaiveDate,
    check_out: NaiveDate,
    rates: &Rates,
    seasons: &[Season],
    catalog: &[Addon],
    chosen: &[(String, i32)],
    deposit_pct: i32,
    max_nights: i32,
) -> Result<Quote, QuoteError> {
    let n = (check_out - check_in).num_days();
    if n < 1 {
        return Err(QuoteError::Dates("check-out must be after check-in".into()));
    }
    if n > max_nights as i64 {
        return Err(QuoteError::TooLong(max_nights as i64));
    }
    let (base, months, weeks, single) = base_for(n, rates).ok_or(QuoteError::NoRate)?;
    let mut pct_sum = 0i64;
    let mut names: Vec<String> = Vec::new();
    let mut min_nights = 1i64;
    for i in 0..n {
        let d = check_in + Duration::days(i);
        if let Some(s) = seasons.iter().find(|s| s.covers(d)) {
            pct_sum += s.adjust_pct as i64;
            min_nights = min_nights.max(s.min_nights as i64);
            if !names.contains(&s.name) {
                names.push(s.name.clone());
            }
        }
    }
    if n < min_nights {
        return Err(QuoteError::TooShort(min_nights));
    }
    let season_pct = (pct_sum as f64 / n as f64).round() as i64;
    let stay = (base as f64 * (100 + season_pct) as f64 / 100.0).round() as i64;
    let mut lines = Vec::new();
    for (key, qty) in chosen {
        let Some(a) = catalog.iter().find(|a| &a.key == key) else {
            continue;
        };
        let qty = (*qty).clamp(1, 20);
        let unit = if a.per == "night" {
            a.price_cents * n
        } else {
            a.price_cents
        };
        lines.push(AddonLine {
            key: a.key.clone(),
            label: a.label.clone(),
            qty,
            cents: unit * qty as i64,
        });
    }
    let total = stay + lines.iter().map(|l| l.cents).sum::<i64>();
    let deposit = (total * deposit_pct.clamp(0, 100) as i64 + 99) / 100;
    Ok(Quote {
        nights: n,
        months,
        weeks,
        single_nights: single,
        base_cents: base,
        season_pct,
        seasons: names,
        stay_cents: stay,
        addons: lines,
        total_cents: total,
        deposit_cents: deposit,
        min_nights,
    })
}

/// Statuses that hold a site.
pub const HOLDING: &[&str] = &["held", "confirmed", "checked_in"];

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn rates() -> Rates {
        Rates {
            night: Some(4_500),
            week: Some(25_000),
            month: Some(80_000),
        }
    }

    #[test]
    fn cheapest_mix() {
        assert_eq!(base_for(3, &rates()), Some((13_500, 0, 0, 3)));
        // 6 nights at $45 is $270; a week is $250.
        assert_eq!(base_for(6, &rates()), Some((25_000, 0, 1, 0)));
        assert_eq!(base_for(9, &rates()), Some((34_000, 0, 1, 2)));
        // 25 nights: a month ($800) beats 3 weeks and 4 nights ($930).
        assert_eq!(base_for(25, &rates()), Some((80_000, 1, 0, 0)));
        assert_eq!(base_for(33, &rates()).map(|b| b.0), Some(93_500));
        let nightly = Rates {
            night: Some(3_000),
            ..Default::default()
        };
        assert_eq!(base_for(10, &nightly), Some((30_000, 0, 0, 10)));
        assert_eq!(base_for(2, &Rates::default()), None);
    }

    #[test]
    fn seasons_wrap_and_adjust() {
        let summer = Season {
            name: "Summer".into(),
            start: "06-01".into(),
            end: "08-31".into(),
            adjust_pct: 20,
            min_nights: 2,
        };
        let winter = Season {
            name: "Winter".into(),
            start: "11-15".into(),
            end: "02-28".into(),
            adjust_pct: -30,
            min_nights: 1,
        };
        assert!(winter.covers(d("2026-01-10")) && winter.covers(d("2026-12-01")));
        assert!(!winter.covers(d("2026-06-10")));
        let q = quote(
            d("2026-07-03"),
            d("2026-07-06"),
            &rates(),
            &[summer.clone(), winter.clone()],
            &[],
            &[],
            25,
            180,
        )
        .ok()
        .unwrap();
        assert_eq!(
            (q.nights, q.base_cents, q.season_pct, q.stay_cents),
            (3, 13_500, 20, 16_200)
        );
        assert_eq!(q.seasons, vec!["Summer"]);
        // Two of four nights in summer: +10% on average.
        let q = quote(
            d("2026-05-30"),
            d("2026-06-03"),
            &rates(),
            std::slice::from_ref(&summer),
            &[],
            &[],
            25,
            180,
        )
        .ok()
        .unwrap();
        assert_eq!(q.season_pct, 10);
        assert!(matches!(
            quote(
                d("2026-07-03"),
                d("2026-07-04"),
                &rates(),
                &[summer],
                &[],
                &[],
                25,
                180
            ),
            Err(QuoteError::TooShort(2))
        ));
    }

    #[test]
    fn addons_deposit_and_limits() {
        let catalog = vec![
            Addon {
                key: "firewood".into(),
                label: "Firewood bundle".into(),
                price_cents: 800,
                per: "stay".into(),
            },
            Addon {
                key: "pet".into(),
                label: "Pet".into(),
                price_cents: 500,
                per: "night".into(),
            },
        ];
        let q = quote(
            d("2026-09-10"),
            d("2026-09-13"),
            &rates(),
            &[],
            &catalog,
            &[
                ("firewood".into(), 2),
                ("pet".into(), 1),
                ("nope".into(), 1),
            ],
            25,
            180,
        )
        .ok()
        .unwrap();
        assert_eq!(q.addons.len(), 2);
        assert_eq!(q.addons[0].cents, 1_600);
        assert_eq!(q.addons[1].cents, 1_500);
        assert_eq!(q.total_cents, 13_500 + 3_100);
        assert_eq!(q.deposit_cents, (16_600 * 25 + 99) / 100);
        assert!(matches!(
            quote(
                d("2026-09-10"),
                d("2026-09-10"),
                &rates(),
                &[],
                &[],
                &[],
                25,
                180
            ),
            Err(QuoteError::Dates(_))
        ));
        assert!(matches!(
            quote(
                d("2026-01-01"),
                d("2026-12-31"),
                &rates(),
                &[],
                &[],
                &[],
                25,
                180
            ),
            Err(QuoteError::TooLong(180))
        ));
    }

    #[test]
    fn month_days() {
        assert!(valid_md("02-29") && valid_md("12-31") && !valid_md("13-01") && !valid_md("2-1"));
    }
}
