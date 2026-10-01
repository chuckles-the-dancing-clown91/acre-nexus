//! Public listing search: filters and sort, kept pure so they are tested
//! without a database.

use super::dto::ListingResp;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Search {
    /// Matches the title, address or city (case-insensitive).
    pub q: Option<String>,
    /// Whole dollars a month.
    pub min_rent: Option<i64>,
    pub max_rent: Option<i64>,
    /// At least this many bedrooms (0 includes studios).
    pub beds: Option<i32>,
    pub baths: Option<i32>,
    pub min_sqft: Option<i32>,
    /// Only homes that can be moved into now.
    pub available_now: bool,
    /// `newest` (default) | `price_asc` | `price_desc` | `beds` | `sqft`.
    pub sort: Option<String>,
}

fn is_now(available_on: &str) -> bool {
    let a = available_on.trim().to_lowercase();
    a.is_empty() || a == "now" || a == "available" || a == "immediately"
}

/// Apply the filters, then the sort. `rows` arrive newest first, which is the
/// default order and the tiebreak for every other sort.
pub fn apply(mut rows: Vec<ListingResp>, s: &Search) -> Vec<ListingResp> {
    if let Some(q) =
        s.q.as_deref()
            .map(|q| q.trim().to_lowercase())
            .filter(|q| !q.is_empty())
    {
        rows.retain(|l| {
            [&l.title, &l.address, &l.city]
                .iter()
                .any(|f| f.to_lowercase().contains(&q))
        });
    }
    if let Some(min) = s.min_rent {
        rows.retain(|l| l.rent_cents >= min * 100);
    }
    if let Some(max) = s.max_rent {
        rows.retain(|l| l.rent_cents <= max * 100);
    }
    if let Some(b) = s.beds {
        rows.retain(|l| l.beds >= b);
    }
    if let Some(b) = s.baths {
        rows.retain(|l| l.baths >= b);
    }
    if let Some(sq) = s.min_sqft {
        rows.retain(|l| l.sqft >= sq);
    }
    if s.available_now {
        rows.retain(|l| is_now(&l.available_on));
    }
    match s.sort.as_deref() {
        Some("price_asc") => rows.sort_by_key(|l| l.rent_cents),
        Some("price_desc") => rows.sort_by_key(|l| std::cmp::Reverse(l.rent_cents)),
        Some("beds") => rows.sort_by_key(|l| std::cmp::Reverse(l.beds)),
        Some("sqft") => rows.sort_by_key(|l| std::cmp::Reverse(l.sqft)),
        _ => {}
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(title: &str, city: &str, beds: i32, rent: i64, on: &str) -> ListingResp {
        ListingResp {
            id: uuid::Uuid::new_v4(),
            title: title.into(),
            address: format!("1 {title} St"),
            city: city.into(),
            beds,
            baths: 1,
            sqft: 800 + beds * 200,
            rent_cents: rent * 100,
            rent_label: String::new(),
            status: "Available".into(),
            available_on: on.into(),
            description: String::new(),
            listed_at: String::new(),
            appliances: vec![],
            upkeep: vec![],
        }
    }

    fn set() -> Vec<ListingResp> {
        vec![
            l("Birch", "Reno", 2, 1800, "Now"),
            l("Cedar", "Reno", 1, 1200, "Jul 15"),
            l("Maple", "Boise", 3, 2400, "Now"),
        ]
    }

    #[test]
    fn filters_combine() {
        let s = Search {
            q: Some("reno".into()),
            max_rent: Some(1900),
            ..Default::default()
        };
        assert_eq!(apply(set(), &s).len(), 2);
        let s = Search {
            beds: Some(2),
            available_now: true,
            ..Default::default()
        };
        let t: Vec<_> = apply(set(), &s).into_iter().map(|l| l.title).collect();
        assert_eq!(t, vec!["Birch", "Maple"]);
        let s = Search {
            min_rent: Some(2000),
            ..Default::default()
        };
        assert_eq!(apply(set(), &s)[0].title, "Maple");
    }

    #[test]
    fn sorts() {
        let s = Search {
            sort: Some("price_asc".into()),
            ..Default::default()
        };
        assert_eq!(apply(set(), &s)[0].title, "Cedar");
        let s = Search {
            sort: Some("price_desc".into()),
            ..Default::default()
        };
        assert_eq!(apply(set(), &s)[0].title, "Maple");
        let s = Search {
            sort: Some("nonsense".into()),
            ..Default::default()
        };
        assert_eq!(apply(set(), &s)[0].title, "Birch");
    }

    #[test]
    fn nothing_matches_cleanly() {
        let s = Search {
            q: Some("zzz".into()),
            ..Default::default()
        };
        assert!(apply(set(), &s).is_empty());
    }
}
