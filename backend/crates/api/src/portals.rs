//! **Listing syndication**: sending the workspace's listings to the rental
//! portals. Portals pull a feed on their own schedule (usually a few times a
//! day) from a URL we give them, so each channel is a secret feed URL:
//!
//! * **Zillow Rental Network** (Zillow, Trulia, HotPads) reads the Zillow
//!   rentals feed, the HotPads 2.1 XML format.
//! * **MITS ILS** is the industry feed Apartments.com and the CoStar network,
//!   Rent., Zumper, ApartmentList and most other listing sites read.
//!
//! A listing goes out when it's public on the website, available, marked for
//! the portals, and complete enough for them to take it (address, state,
//! ZIP, rent, a description, at least one photo). [`readiness`] says what's
//! missing, listing by listing. Turning a channel on doesn't send anything
//! by itself: the portal has to be given the URL, under an agreement with
//! them. Every pull is logged, so the console shows when each portal last
//! came by.

use chrono::{Datelike, NaiveDate, Utc};
use serde::Serialize;

#[derive(Serialize, Clone, Copy, schemars::JsonSchema)]
pub struct ChannelDef {
    pub key: &'static str,
    pub label: &'static str,
    /// Where the listings show up.
    pub reaches: &'static str,
    pub format: &'static str,
    /// How to get the portal reading the feed.
    pub how_to: &'static str,
}

pub const CHANNELS: &[ChannelDef] = &[
    ChannelDef {
        key: "zillow",
        label: "Zillow Rental Network",
        reaches: "Zillow, Trulia and HotPads",
        format: "Zillow rentals feed (HotPads 2.1 XML)",
        how_to: "Apply as a feed partner with Zillow Rentals (Zillow Rental Manager → Listing feeds) \
                 and give them this URL. Zillow checks the first pull, then reads it a few times a day.",
    },
    ChannelDef {
        key: "mits",
        label: "MITS ILS feed",
        reaches: "Apartments.com and the CoStar network, Rent., Zumper, ApartmentList and other listing sites",
        format: "MITS 4.1 ILS XML",
        how_to: "Each site onboards feeds through its own partner or support team: send them this URL \
                 and your company name. One URL serves them all.",
    },
];

pub fn channel(key: &str) -> Option<&'static ChannelDef> {
    CHANNELS.iter().find(|c| c.key == key)
}

/// Everything a feed says about one listing.
#[derive(Clone)]
pub struct FeedListing {
    pub id: uuid::Uuid,
    pub property_id: Option<uuid::Uuid>,
    pub property_name: String,
    pub property_type: String,
    pub title: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub beds: i32,
    pub baths: i32,
    pub sqft: i32,
    pub rent_cents: i64,
    pub description: String,
    pub available: String,
    pub photos: Vec<(String, Option<String>)>,
    pub website: Option<String>,
    pub updated: String,
    pub status: String,
    pub is_public: bool,
    pub syndicate: bool,
}

/// Who renters reach, from the channel (or the business profile).
#[derive(Clone, Default)]
pub struct Contact {
    pub company: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub website: String,
}

#[derive(Serialize, Clone, schemars::JsonSchema)]
pub struct Issue {
    pub message: String,
    /// Kept out of the feed until fixed (otherwise just advice).
    pub blocking: bool,
}

fn issue(m: &str, blocking: bool) -> Issue {
    Issue {
        message: m.into(),
        blocking,
    }
}

/// What stands between a listing and the portals.
pub fn readiness(l: &FeedListing) -> Vec<Issue> {
    let mut out = vec![];
    if !l.is_public {
        out.push(issue("Not published on your website", true));
    }
    if matches!(l.status.as_str(), "Pending" | "Leased") {
        out.push(issue(
            &format!("{}, so it isn't advertised", l.status),
            true,
        ));
    }
    if !l.syndicate {
        out.push(issue("Turned off for the portals", true));
    }
    if l.address.trim().is_empty() || l.city.trim().is_empty() {
        out.push(issue("Needs a street address and city", true));
    }
    if l.state.len() != 2 {
        out.push(issue("Needs a two-letter state", true));
    }
    let zip: String = l.zip.chars().filter(|c| c.is_ascii_digit()).collect();
    if zip.len() != 5 && zip.len() != 9 {
        out.push(issue("Needs a ZIP code", true));
    }
    if l.rent_cents <= 0 {
        out.push(issue("Needs the rent", true));
    }
    if l.photos.is_empty() {
        out.push(issue(
            "Needs at least one photo; portals drop listings without one",
            true,
        ));
    }
    let words = l.description.split_whitespace().count();
    if words == 0 {
        out.push(issue("Needs a description", true));
    } else if words < 30 {
        out.push(issue(
            "A longer description (30 words or more) ranks better",
            false,
        ));
    }
    if l.photos.len() == 1 || l.photos.len() == 2 {
        out.push(issue(
            "Listings with five or more photos get more inquiries",
            false,
        ));
    }
    if l.sqft <= 0 {
        out.push(issue("Add the square footage", false));
    }
    out
}

pub fn ready(l: &FeedListing) -> bool {
    readiness(l).iter().all(|i| !i.blocking)
}

/// Feed-wide problems: renters need someone to reach.
pub fn contact_issues(c: &Contact) -> Vec<Issue> {
    let mut out = vec![];
    if c.email.trim().is_empty() && c.phone.trim().is_empty() {
        out.push(issue(
            "Add a leasing email or phone; every listing shows it",
            true,
        ));
    }
    if c.company.trim().is_empty() {
        out.push(issue(
            "Add your company name (Settings → Business profile)",
            false,
        ));
    }
    out
}

/// The availability label staff type ("Now", "Aug 1", "2026-08-01", "8/1")
/// as a date, today when it's now or can't be read.
pub fn available_date(label: &str, today: NaiveDate) -> NaiveDate {
    let t = label.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("now") || t.eq_ignore_ascii_case("immediately") {
        return today;
    }
    if let Some(iso) = crate::imports::date(t) {
        if let Ok(d) = NaiveDate::parse_from_str(&iso, "%Y-%m-%d") {
            return d.max(today);
        }
    }
    // "Aug 1", "August 1", "8/1": the next one.
    let year = today.year();
    let parsed = ["%b %d %Y", "%B %d %Y", "%m/%d %Y"]
        .iter()
        .find_map(|f| NaiveDate::parse_from_str(&format!("{t} {year}"), f).ok());
    match parsed {
        Some(d) if d < today => d.with_year(year + 1).unwrap_or(d),
        Some(d) => d,
        None => today,
    }
}

fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn el(out: &mut String, indent: usize, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    out.push_str(&" ".repeat(indent));
    out.push_str(&format!("<{name}>{}</{name}>\n", esc(value)));
}

fn whole_dollars(cents: i64) -> String {
    format!("{}", (cents + 50) / 100)
}

/// The Zillow rentals feed (HotPads 2.1).
pub fn zillow_feed(company_id: &str, contact: &Contact, listings: &[FeedListing]) -> String {
    let today = Utc::now().date_naive();
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<hotPadsItems version=\"2.1\">\n",
    );
    x.push_str(&format!("  <Company id=\"{}\">\n", esc(company_id)));
    el(&mut x, 4, "name", &contact.company);
    el(&mut x, 4, "website", &contact.website);
    x.push_str("  </Company>\n");
    for l in listings {
        let ptype = match l.property_type.to_lowercase() {
            t if t.contains("condo") => "CONDO",
            t if t.contains("town") => "TOWNHOUSE",
            t if t.contains("single") || t.contains("house") || t.contains("sfr") => "HOUSE",
            _ => "LARGE",
        };
        x.push_str(&format!(
            "  <Listing id=\"{}\" type=\"RENTAL\" companyId=\"{}\" propertyType=\"{ptype}\">\n",
            l.id,
            esc(company_id)
        ));
        el(&mut x, 4, "name", &l.title);
        x.push_str(&format!(
            "    <street hide=\"false\">{}</street>\n",
            esc(&l.address)
        ));
        el(&mut x, 4, "city", &l.city);
        el(&mut x, 4, "state", &l.state);
        el(&mut x, 4, "zip", &l.zip);
        el(&mut x, 4, "country", "US");
        if let (Some(lat), Some(lng)) = (l.latitude, l.longitude) {
            el(&mut x, 4, "latitude", &format!("{lat:.6}"));
            el(&mut x, 4, "longitude", &format!("{lng:.6}"));
        }
        el(&mut x, 4, "lastUpdated", &l.updated);
        el(&mut x, 4, "contactName", &contact.name);
        el(&mut x, 4, "contactEmail", &contact.email);
        el(&mut x, 4, "contactPhone", &contact.phone);
        el(&mut x, 4, "description", &l.description);
        el(
            &mut x,
            4,
            "website",
            l.website.as_deref().unwrap_or(&contact.website),
        );
        for (src, caption) in &l.photos {
            x.push_str(&format!("    <ListingPhoto source=\"{}\">", esc(src)));
            if let Some(c) = caption.as_deref().filter(|c| !c.is_empty()) {
                x.push_str(&format!("<caption>{}</caption>", esc(c)));
            }
            x.push_str("</ListingPhoto>\n");
        }
        el(&mut x, 4, "price", &whole_dollars(l.rent_cents));
        el(&mut x, 4, "pricingFrequency", "MONTHLY");
        el(&mut x, 4, "numBedrooms", &l.beds.max(0).to_string());
        el(&mut x, 4, "numFullBaths", &l.baths.max(0).to_string());
        el(&mut x, 4, "numHalfBaths", "0");
        if l.sqft > 0 {
            el(&mut x, 4, "squareFeet", &l.sqft.to_string());
        }
        el(
            &mut x,
            4,
            "dateAvailable",
            &available_date(&l.available, today).to_string(),
        );
        x.push_str("  </Listing>\n");
    }
    x.push_str("</hotPadsItems>\n");
    x
}

/// The MITS 4.1 ILS feed: one `Property` per building, each listing a unit
/// on its own floor plan.
pub fn mits_feed(company_id: &str, contact: &Contact, listings: &[FeedListing]) -> String {
    let today = Utc::now().date_naive();
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<PhysicalProperty xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    x.push_str(&format!("  <Management IDValue=\"{}\">\n", esc(company_id)));
    el(&mut x, 4, "Name", &contact.company);
    el(&mut x, 4, "WebSite", &contact.website);
    x.push_str("  </Management>\n");
    // Group by property; a listing with no property is its own building.
    let mut groups: Vec<(String, Vec<&FeedListing>)> = vec![];
    for l in listings {
        let key = l.property_id.unwrap_or(l.id).to_string();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(l),
            None => groups.push((key, vec![l])),
        }
    }
    for (pid, ls) in groups {
        let first = ls[0];
        x.push_str(&format!("  <Property IDValue=\"{}\">\n", esc(&pid)));
        x.push_str("    <PropertyID>\n");
        x.push_str(&format!(
            "      <Identification IDValue=\"{}\" OrganizationName=\"{}\" IDType=\"PropertyID\"/>\n",
            esc(&pid),
            esc(&contact.company)
        ));
        el(
            &mut x,
            6,
            "MarketingName",
            if first.property_name.is_empty() {
                &first.title
            } else {
                &first.property_name
            },
        );
        el(&mut x, 6, "WebSite", &contact.website);
        x.push_str("      <Address AddressType=\"property\">\n");
        el(&mut x, 8, "AddressLine1", &first.address);
        el(&mut x, 8, "City", &first.city);
        el(&mut x, 8, "State", &first.state);
        el(&mut x, 8, "PostalCode", &first.zip);
        el(&mut x, 8, "Country", "US");
        x.push_str("      </Address>\n");
        if !contact.phone.is_empty() {
            x.push_str("      <Phone PhoneType=\"office\">\n");
            el(&mut x, 8, "PhoneNumber", &contact.phone);
            x.push_str("      </Phone>\n");
        }
        el(&mut x, 6, "Email", &contact.email);
        x.push_str("    </PropertyID>\n");
        x.push_str("    <ILS_Identification ILS_IdentificationType=\"Apartment\" RentalType=\"Market Rate\">\n");
        if let (Some(lat), Some(lng)) = (first.latitude, first.longitude) {
            el(&mut x, 6, "Latitude", &format!("{lat:.6}"));
            el(&mut x, 6, "Longitude", &format!("{lng:.6}"));
        }
        x.push_str("    </ILS_Identification>\n");
        x.push_str("    <Information>\n");
        el(
            &mut x,
            6,
            "StructureType",
            if first.property_type.is_empty() {
                "Apartment"
            } else {
                &first.property_type
            },
        );
        el(&mut x, 6, "LongDescription", &first.description);
        x.push_str("    </Information>\n");
        for l in &ls {
            let fp = format!("fp-{}", l.id);
            let rent = whole_dollars(l.rent_cents);
            x.push_str(&format!("    <Floorplan IDValue=\"{fp}\">\n"));
            el(&mut x, 6, "Name", &l.title);
            el(&mut x, 6, "UnitCount", "1");
            el(&mut x, 6, "UnitsAvailable", "1");
            x.push_str(&format!(
                "      <Room RoomType=\"Bedroom\"><Count>{}</Count></Room>\n      <Room RoomType=\"Bathroom\"><Count>{}</Count></Room>\n",
                l.beds.max(0),
                l.baths.max(0)
            ));
            if l.sqft > 0 {
                x.push_str(&format!(
                    "      <SquareFeet Min=\"{0}\" Max=\"{0}\"/>\n",
                    l.sqft
                ));
            }
            x.push_str(&format!(
                "      <MarketRent Min=\"{rent}\" Max=\"{rent}\"/>\n"
            ));
            x.push_str(&format!(
                "      <EffectiveRent Min=\"{rent}\" Max=\"{rent}\"/>\n"
            ));
            for (i, (src, caption)) in l.photos.iter().enumerate() {
                x.push_str(&format!(
                    "      <File Active=\"true\" FileID=\"{}-{i}\">\n",
                    l.id
                ));
                el(&mut x, 8, "FileType", "Photo");
                el(&mut x, 8, "Caption", caption.as_deref().unwrap_or(""));
                el(&mut x, 8, "Src", src);
                el(&mut x, 8, "Rank", &(i + 1).to_string());
                x.push_str("      </File>\n");
            }
            x.push_str("    </Floorplan>\n");
            let d = available_date(&l.available, today);
            x.push_str(&format!(
                "    <ILS_Unit IDValue=\"{}\">\n      <Units>\n        <Unit>\n",
                l.id
            ));
            x.push_str(&format!(
                "          <Identification IDValue=\"{}\" OrganizationName=\"{}\"/>\n",
                l.id,
                esc(&contact.company)
            ));
            el(&mut x, 10, "MarketingName", &l.title);
            el(&mut x, 10, "UnitBedrooms", &l.beds.max(0).to_string());
            el(&mut x, 10, "UnitBathrooms", &l.baths.max(0).to_string());
            if l.sqft > 0 {
                el(&mut x, 10, "MinSquareFeet", &l.sqft.to_string());
                el(&mut x, 10, "MaxSquareFeet", &l.sqft.to_string());
            }
            el(&mut x, 10, "UnitRent", &rent);
            el(&mut x, 10, "MarketRent", &rent);
            x.push_str(&format!("          <FloorplanID>{fp}</FloorplanID>\n"));
            x.push_str("        </Unit>\n      </Units>\n      <Availability>\n");
            x.push_str(&format!(
                "        <MadeReadyDate Month=\"{}\" Day=\"{}\" Year=\"{}\"/>\n",
                d.month(),
                d.day(),
                d.year()
            ));
            el(&mut x, 8, "VacancyClass", "Unoccupied");
            x.push_str("      </Availability>\n");
            x.push_str(&format!(
                "      <EffectiveRent Min=\"{rent}\" Max=\"{rent}\"/>\n"
            ));
            x.push_str("    </ILS_Unit>\n");
        }
        x.push_str("  </Property>\n");
    }
    x.push_str("</PhysicalProperty>\n");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> FeedListing {
        FeedListing {
            id: uuid::Uuid::nil(),
            property_id: None,
            property_name: "Maple Court".into(),
            property_type: "Multifamily".into(),
            title: "Bright 2-bed at Maple Court".into(),
            address: "123 Maple Ct".into(),
            city: "Portland".into(),
            state: "OR".into(),
            zip: "97214".into(),
            latitude: Some(45.5),
            longitude: Some(-122.6),
            beds: 2,
            baths: 1,
            sqft: 880,
            rent_cents: 185_000,
            description: "Hardwood floors & in-unit laundry <3".into(),
            available: "Now".into(),
            photos: vec![("https://api.example/p/1".into(), Some("Living room".into()))],
            website: None,
            updated: "2026-10-03T12:00:00".into(),
            status: "Available".into(),
            is_public: true,
            syndicate: true,
        }
    }

    fn contact() -> Contact {
        Contact {
            company: "Northwind Property Group".into(),
            name: "Leasing office".into(),
            email: "leasing@northwind.example".into(),
            phone: "(503) 555-0100".into(),
            website: "https://northwind.example".into(),
        }
    }

    #[test]
    fn a_complete_listing_is_ready() {
        let l = listing();
        assert!(
            ready(&l),
            "{:?}",
            readiness(&l).iter().map(|i| &i.message).collect::<Vec<_>>()
        );
        // Advice, not a block.
        assert!(readiness(&l)
            .iter()
            .any(|i| !i.blocking && i.message.contains("description")));
    }

    #[test]
    fn what_keeps_a_listing_off_the_portals() {
        let mut l = listing();
        l.photos.clear();
        l.zip = "".into();
        l.status = "Leased".into();
        let msgs: Vec<String> = readiness(&l)
            .into_iter()
            .filter(|i| i.blocking)
            .map(|i| i.message)
            .collect();
        assert_eq!(msgs.len(), 3, "{msgs:?}");
        assert!(!ready(&l));
        assert_eq!(
            contact_issues(&Contact::default())
                .iter()
                .filter(|i| i.blocking)
                .count(),
            1
        );
    }

    #[test]
    fn availability_labels_become_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();
        assert_eq!(available_date("Now", today), today);
        assert_eq!(available_date("Nov 15", today).to_string(), "2026-11-15");
        assert_eq!(
            available_date("Jul 1", today).to_string(),
            "2027-07-01",
            "next July"
        );
        assert_eq!(
            available_date("2026-12-01", today).to_string(),
            "2026-12-01"
        );
        assert_eq!(
            available_date("2025-01-01", today),
            today,
            "a past date is now"
        );
        assert_eq!(available_date("whenever", today), today);
    }

    #[test]
    fn feeds_are_well_formed_and_escaped() {
        let z = zillow_feed("northwind", &contact(), &[listing()]);
        assert!(z.starts_with("<?xml"));
        assert!(z.contains("<hotPadsItems version=\"2.1\">"));
        assert!(z.contains("<price>1850</price>"));
        assert!(z.contains("Hardwood floors &amp; in-unit laundry &lt;3"));
        assert!(z.contains("<ListingPhoto source=\"https://api.example/p/1\"><caption>Living room</caption></ListingPhoto>"));
        assert_eq!(
            z.matches("<Listing ").count(),
            z.matches("</Listing>").count()
        );
        let m = mits_feed("northwind", &contact(), &[listing(), listing()]);
        assert!(m.contains("<PhysicalProperty"));
        assert_eq!(m.matches("<Property IDValue").count(), 1, "one building");
        assert_eq!(m.matches("<ILS_Unit ").count(), 2);
        assert!(m.contains("<MarketRent Min=\"1850\" Max=\"1850\"/>"));
    }
}
