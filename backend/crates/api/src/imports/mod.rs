//! **Imports**: bringing a workspace over from another property management
//! tool. Every tool can export a rent roll, a property and unit list, owners
//! and vendors as CSV; this reads those files, works out which columns mean
//! what, and turns the rows into properties, units, leases, owners and
//! vendors.
//!
//! The flow is upload → map → preview → commit, and undo:
//!
//! * **Upload** keeps the file on an `import_batch` draft and guesses the
//!   source tool and the column mapping from the header row.
//! * **Map**: the guess can be changed field by field.
//! * **Preview** runs the real import inside a savepoint and rolls it back, so
//!   what it shows is exactly what a commit does ([`apply`]).
//! * **Commit** runs it for real, row by row, each row in its own savepoint so
//!   one bad row doesn't sink the rest. The batch keeps what it created.
//! * **Undo** removes what the batch created, unless something has happened
//!   to it since ([`undo`]).
//!
//! This file is the pure part: decoding, header matching, source detection,
//! and cleaning values (money, dates, names, beds/baths).

pub mod apply;
pub mod export;
pub mod undo;

use chrono::NaiveDate;
use serde::Serialize;
use std::collections::BTreeMap;

/// Largest file accepted (10 MiB).
pub const MAX_BYTES: usize = 10 * 1024 * 1024;
/// Most rows in one import; bigger files are split.
pub const MAX_ROWS: usize = 5_000;

pub const KINDS: &[&str] = &["properties", "tenants", "owners", "vendors"];

/// One field the importer understands for a kind.
#[derive(Serialize, Clone, Copy, schemars::JsonSchema)]
pub struct FieldDef {
    pub key: &'static str,
    pub label: &'static str,
    pub required: bool,
    pub hint: &'static str,
}

const fn f(key: &'static str, label: &'static str, required: bool, hint: &'static str) -> FieldDef {
    FieldDef {
        key,
        label,
        required,
        hint,
    }
}

const PROPERTIES_FIELDS: &[FieldDef] = &[
    f(
        "property",
        "Property",
        true,
        "Name, or the street address if there's no name",
    ),
    f(
        "address",
        "Street address",
        false,
        "Defaults to the property name",
    ),
    f("city", "City", false, ""),
    f("state", "State", false, "Two letters or the full name"),
    f("zip", "ZIP", false, ""),
    f(
        "property_type",
        "Property type",
        false,
        "e.g. Single family, Multifamily",
    ),
    f("year_built", "Year built", false, ""),
    f(
        "unit",
        "Unit",
        false,
        "One row per unit; leave blank for a single-family home",
    ),
    f("beds", "Beds", false, "0 for a studio"),
    f("baths", "Baths", false, "e.g. 1.5"),
    f(
        "bd_ba",
        "Beds/Baths",
        false,
        "One column like 2/1.5 (AppFolio)",
    ),
    f("sqft", "Square feet", false, ""),
    f("market_rent", "Market rent", false, "$1,250.00"),
];

const TENANTS_FIELDS: &[FieldDef] = &[
    f(
        "property",
        "Property",
        true,
        "Name or street address; created if new",
    ),
    f("address", "Street address", false, ""),
    f("city", "City", false, ""),
    f("state", "State", false, ""),
    f("zip", "ZIP", false, ""),
    f("unit", "Unit", false, "Created if new"),
    f(
        "tenant_name",
        "Tenant",
        true,
        "Or first and last name columns",
    ),
    f("first_name", "First name", false, ""),
    f("last_name", "Last name", false, ""),
    f(
        "email",
        "Email",
        false,
        "The first one if there are several",
    ),
    f("phone", "Phone", false, ""),
    f("rent", "Rent", true, "Monthly rent"),
    f("deposit", "Deposit", false, "Security deposit held"),
    f("lease_start", "Lease start", false, "Or move-in date"),
    f("lease_end", "Lease end", false, "Blank for month-to-month"),
    f(
        "move_in",
        "Move-in",
        false,
        "Used when there's no lease start",
    ),
    f(
        "balance",
        "Balance",
        false,
        "What they owe today; negative is a credit",
    ),
    f("status", "Status", false, "Current, notice, future, past"),
    f("beds", "Beds", false, "For a unit created by the import"),
    f("baths", "Baths", false, ""),
    f("bd_ba", "Beds/Baths", false, "2/1.5"),
    f("sqft", "Square feet", false, ""),
];

const OWNERS_FIELDS: &[FieldDef] = &[
    f(
        "name",
        "Owner",
        true,
        "Person or company; or first and last name",
    ),
    f("first_name", "First name", false, ""),
    f("last_name", "Last name", false, ""),
    f("company", "Company", false, "Makes the owner a company"),
    f("email", "Email", false, ""),
    f("phone", "Phone", false, ""),
    f("notes", "Notes", false, ""),
];

const VENDORS_FIELDS: &[FieldDef] = &[
    f("name", "Vendor", true, "Company name"),
    f("contact_name", "Contact", false, ""),
    f("email", "Email", false, ""),
    f("phone", "Phone", false, ""),
    f("website", "Website", false, ""),
    f("address", "Address", false, ""),
    f("trades", "Trades", false, "e.g. Plumbing; HVAC"),
    f("notes", "Notes", false, ""),
];

/// The fields for each kind, in the order the template and the export use.
pub fn fields(kind: &str) -> &'static [FieldDef] {
    match kind {
        "properties" => PROPERTIES_FIELDS,
        "tenants" => TENANTS_FIELDS,
        "owners" => OWNERS_FIELDS,
        "vendors" => VENDORS_FIELDS,
        _ => &[],
    }
}

/// A tool whose exports we recognise.
#[derive(Serialize, Clone, Copy, schemars::JsonSchema)]
pub struct Source {
    pub key: &'static str,
    pub label: &'static str,
    /// Where to find the export in that tool.
    pub how: &'static str,
}

pub const SOURCES: &[Source] = &[
    Source {
        key: "appfolio",
        label: "AppFolio",
        how: "Reports → Rent Roll (or Property Directory, Owner Directory, Vendor Directory) → Export → CSV.",
    },
    Source {
        key: "buildium",
        label: "Buildium",
        how: "Reports → Rent roll (or Rental properties, Rental owners, Vendors) → Export → CSV.",
    },
    Source {
        key: "yardi",
        label: "Yardi Breeze",
        how: "Reports → Rent Roll (or Properties, Owners, Vendors) → Export to Excel, then save as CSV.",
    },
    Source {
        key: "rentmanager",
        label: "Rent Manager",
        how: "Reports → Rent Roll → Export → CSV.",
    },
    Source {
        key: "doorloop",
        label: "DoorLoop",
        how: "Reports → Rent Roll (or Properties, Owners, Vendors) → Export → CSV.",
    },
    Source {
        key: "vantedge",
        label: "Vantedge export",
        how: "A file from Import & export → Export in another Vantedge workspace.",
    },
    Source {
        key: "generic",
        label: "Spreadsheet",
        how: "Any CSV with a header row. Start from our template if you're building one by hand.",
    },
];

/// `"Lease From"` → `"leasefrom"`: lowercase letters and digits only, so
/// headers match whatever their spacing, case and punctuation.
pub fn norm(h: &str) -> String {
    h.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Header spellings for each field, normalised, best first.
fn aliases(kind: &str, field: &str) -> &'static [&'static str] {
    match (kind, field) {
        (_, "property") => &[
            "property",
            "propertyname",
            "propname",
            "building",
            "buildingname",
            "propertyshortname",
            "rentalproperty",
            "community",
        ],
        (_, "address") => &[
            "address",
            "streetaddress",
            "propertyaddress",
            "street",
            "address1",
            "addressline1",
            "propertystreetaddress1",
            "propertystreet",
        ],
        (_, "city") => &["city", "propertycity", "town"],
        (_, "state") => &[
            "state",
            "propertystate",
            "stateprovince",
            "province",
            "region",
        ],
        (_, "zip") => &[
            "zip",
            "zipcode",
            "postalcode",
            "postcode",
            "propertyzip",
            "propertyzipcode",
            "postal",
        ],
        (_, "property_type") => &["propertytype", "type", "propertysubtype"],
        (_, "year_built") => &["yearbuilt", "built"],
        (_, "unit") => &[
            "unit",
            "unitnumber",
            "unitname",
            "unitno",
            "apt",
            "apartment",
            "suite",
            "unitid",
        ],
        (_, "beds") => &["beds", "bedrooms", "bed", "bd", "br", "unitbedrooms"],
        (_, "baths") => &["baths", "bathrooms", "bath", "ba", "unitbathrooms"],
        (_, "bd_ba") => &[
            "bdba",
            "bedbath",
            "bedsbaths",
            "bedroomsbathrooms",
            "bdbath",
        ],
        (_, "sqft") => &[
            "sqft",
            "squarefeet",
            "squarefootage",
            "sqfeet",
            "size",
            "area",
            "unitsqft",
        ],
        ("properties", "market_rent") => &[
            "marketrent",
            "rent",
            "askingrent",
            "unitrent",
            "monthlyrent",
            "targetrent",
            "listrent",
        ],
        (_, "tenant_name") => &[
            "tenant",
            "tenantname",
            "tenants",
            "tenantsnames",
            "resident",
            "residentname",
            "residents",
            "tenantdisplayname",
            "leaseholder",
            "occupant",
            "name",
        ],
        (_, "first_name") => &["firstname", "first", "givenname"],
        (_, "last_name") => &["lastname", "last", "surname", "familyname"],
        (_, "email") => &[
            "email",
            "emailaddress",
            "primaryemail",
            "tenantemail",
            "emails",
            "contactemail",
        ],
        (_, "phone") => &[
            "phone",
            "phonenumber",
            "mobile",
            "mobilephone",
            "cell",
            "cellphone",
            "primaryphone",
            "tenantphone",
            "phonenumbers",
            "contactphone",
            "homephone",
        ],
        ("tenants", "rent") => &[
            "rent",
            "monthlyrent",
            "leaserent",
            "currentrent",
            "rentamount",
            "actualrent",
            "chargedrent",
            "scheduledrent",
            "marketrent",
        ],
        (_, "deposit") => &[
            "deposit",
            "securitydeposit",
            "deposits",
            "depositheld",
            "depositsheld",
            "securitydepositheld",
            "deposit1",
        ],
        (_, "lease_start") => &[
            "leasestart",
            "leasefrom",
            "leasestartdate",
            "startdate",
            "leasebegin",
            "from",
            "start",
        ],
        (_, "lease_end") => &[
            "leaseend",
            "leaseto",
            "leaseenddate",
            "enddate",
            "leaseexpiration",
            "leaseexpirationdate",
            "expiration",
            "to",
            "end",
        ],
        (_, "move_in") => &["movein", "moveindate", "moveinon"],
        (_, "balance") => &[
            "balance",
            "pastdue",
            "balancedue",
            "amountdue",
            "currentbalance",
            "outstandingbalance",
            "totalbalance",
            "openbalance",
            "amountowed",
        ],
        (_, "status") => &[
            "status",
            "leasestatus",
            "tenantstatus",
            "residentstatus",
            "occupancy",
        ],
        ("owners", "name") => &[
            "owner",
            "ownername",
            "name",
            "rentalowner",
            "rentalowners",
            "fullname",
            "displayname",
            "payeename",
        ],
        (_, "company") => &[
            "company",
            "companyname",
            "business",
            "businessname",
            "entity",
        ],
        ("vendors", "name") => &[
            "vendor",
            "vendorname",
            "company",
            "companyname",
            "name",
            "businessname",
            "payeename",
            "displayname",
        ],
        (_, "contact_name") => &["contact", "contactname", "primarycontact", "contactperson"],
        (_, "website") => &["website", "web", "url", "site"],
        (_, "trades") => &[
            "trades",
            "trade",
            "category",
            "categories",
            "vendortype",
            "specialty",
            "specialties",
            "service",
            "services",
            "vendorcategory",
        ],
        (_, "notes") => &["notes", "note", "comments", "comment", "memo"],
        _ => &[],
    }
}

/// Guess which column holds each field. Exact alias matches win, best alias
/// first; a column is used once. In a tenants file the `name` alias only
/// counts when there's no clearer tenant column.
pub fn auto_map(kind: &str, headers: &[String]) -> BTreeMap<String, String> {
    let normed: Vec<String> = headers.iter().map(|h| norm(h)).collect();
    let mut used = vec![false; headers.len()];
    let mut out = BTreeMap::new();
    for fd in fields(kind) {
        for alias in aliases(kind, fd.key) {
            if let Some(i) = normed.iter().position(|h| h == alias) {
                if !used[i] {
                    used[i] = true;
                    out.insert(fd.key.to_string(), headers[i].clone());
                    break;
                }
            }
        }
    }
    out
}

/// Required fields with no column, allowing for the stand-ins: an address
/// for the property, first and last names for a name, a company for an
/// owner's name, a move-in date for the lease start.
pub fn missing_required(kind: &str, mapping: &BTreeMap<String, String>) -> Vec<&'static str> {
    let has = |k: &str| mapping.get(k).is_some_and(|v| !v.is_empty());
    fields(kind)
        .iter()
        .filter(|fd| fd.required)
        .filter(|fd| match fd.key {
            "property" => !has("property") && !has("address"),
            "tenant_name" => !has("tenant_name") && !(has("first_name") || has("last_name")),
            "name" => !has("name") && !has("company") && !(has("first_name") || has("last_name")),
            k => !has(k),
        })
        .map(|fd| fd.label)
        .collect()
}

/// Columns that point to one tool more than the others. A guess, shown to
/// the person importing, never a reason to refuse a file.
const SIGNATURES: &[(&str, &[&str])] = &[
    ("vantedge", &["vantedgeid"]),
    (
        "appfolio",
        &[
            "leasefrom",
            "leaseto",
            "bdba",
            "pastdue",
            "unittags",
            "nsfcount",
            "lateCount",
        ],
    ),
    (
        "buildium",
        &[
            "leaseid",
            "rentalowner",
            "rentalowners",
            "leasetype",
            "rentalproperty",
            "tenantsnames",
        ],
    ),
    (
        "yardi",
        &[
            "residentcode",
            "tenantcode",
            "tcode",
            "leaseexpiration",
            "resident",
            "unittype",
        ],
    ),
    (
        "rentmanager",
        &[
            "propertyshortname",
            "tenantdisplayname",
            "unitname",
            "tenantid",
            "propname",
        ],
    ),
    (
        "doorloop",
        &[
            "leasename",
            "tenantsnames",
            "leaseterm",
            "outstandingbalance",
        ],
    ),
];

/// The tool a file most likely came from, by its headers.
pub fn detect_source(headers: &[String]) -> &'static str {
    let normed: Vec<String> = headers.iter().map(|h| norm(h)).collect();
    let mut best = ("generic", 0usize);
    for (key, sig) in SIGNATURES {
        let score = sig
            .iter()
            .filter(|s| normed.iter().any(|h| *h == norm(s)))
            .count();
        let needed = if *key == "vantedge" { 1 } else { 2 };
        if score >= needed && score > best.1 {
            best = (key, score);
        }
    }
    best.0
}

// ---------------------------------------------------------------------------
// Reading the file
// ---------------------------------------------------------------------------

/// The file as text. Most exports are UTF-8; Excel's "CSV" is often
/// Windows-1252, so bytes that aren't UTF-8 are read that way.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// A parsed file: the header row and the data rows (blank rows dropped).
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Each row's line in the file, for messages a person can find.
    pub lines: Vec<usize>,
}

/// Parse CSV text. Some reports put a title and blank lines above the real
/// header row; the header is the first row with at least two filled cells
/// that is followed by data. Tab-separated files work too.
pub fn parse(text: &str) -> Result<Table, String> {
    let delim = if text.lines().next().unwrap_or("").matches('\t').count()
        > text.lines().next().unwrap_or("").matches(',').count()
    {
        b'\t'
    } else {
        b','
    };
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delim)
        .from_reader(text.as_bytes());
    let mut all: Vec<(usize, Vec<String>)> = vec![];
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("couldn't read the file as CSV: {e}"))?;
        let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
        let row: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
        if row.iter().all(|c| c.is_empty()) {
            continue;
        }
        all.push((line, row));
    }
    let filled = |r: &Vec<String>| r.iter().filter(|c| !c.is_empty()).count();
    let start = all
        .iter()
        .position(|(_, r)| filled(r) >= 2)
        .ok_or("the file has no header row")?;
    let headers = all[start].1.clone();
    let width = headers.len();
    let (lines, rows): (Vec<usize>, Vec<Vec<String>>) = all
        .into_iter()
        .skip(start + 1)
        .map(|(line, mut r)| {
            r.resize(width, String::new());
            (line, r)
        })
        // Report footers: a "Total" row, or a row filled in one cell only.
        .filter(|(_, r)| {
            let first = r.iter().find(|c| !c.is_empty()).map(|c| norm(c));
            filled(r) >= 2 && !matches!(first.as_deref(), Some("total" | "totals" | "grandtotal"))
        })
        .unzip();
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "the file has {} rows; split it into files of {MAX_ROWS} or fewer",
            rows.len()
        ));
    }
    Ok(Table {
        headers,
        rows,
        lines,
    })
}

/// One row's value for a field, through the mapping.
pub fn cell<'a>(
    t: &'a Table,
    row: &'a [String],
    mapping: &BTreeMap<String, String>,
    field: &str,
) -> Option<&'a str> {
    let h = mapping.get(field)?;
    let i = t.headers.iter().position(|x| x == h)?;
    row.get(i)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "-" && *s != "--")
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

/// `"$1,250.00"` → 125000; `"(45.10)"` → -4510; blank → None.
pub fn money(s: &str) -> Option<i64> {
    let t = s.trim();
    let neg = (t.starts_with('(') && t.ends_with(')')) || t.starts_with('-') || t.ends_with('-');
    let digits: String = t
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.is_empty() || digits == "." {
        return None;
    }
    let v: f64 = digits.parse().ok()?;
    let cents = (v * 100.0).round() as i64;
    Some(if neg { -cents } else { cents })
}

/// Dates as other tools write them, to ISO: `2024-07-01`, `7/1/2024`,
/// `07/01/24`, `1-Jul-2024`, `Jul 1, 2024`. Two-digit years are 2000s up to
/// 69, 1900s after.
pub fn date(s: &str) -> Option<String> {
    let t = s.trim();
    let t = t
        .split(['T', ' '])
        .next()
        .filter(|p| p.contains(['-', '/']))
        .unwrap_or(t);
    let parts: Vec<&str> = t.split(['/', '-']).collect();
    if parts.len() == 3 && parts[2].len() == 2 && parts[0].len() <= 2 {
        let y: i32 = parts[2].parse().ok()?;
        let y = if y < 70 { 2000 + y } else { 1900 + y };
        let d = NaiveDate::from_ymd_opt(y, parts[0].parse().ok()?, parts[1].parse().ok()?)?;
        return Some(d.format("%Y-%m-%d").to_string());
    }
    for fmt in [
        "%Y-%m-%d",
        "%m/%d/%Y",
        "%d-%b-%Y",
        "%b %d, %Y",
        "%B %d, %Y",
        "%Y/%m/%d",
        "%m-%d-%Y",
    ] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    // "Jul 1, 2024" written without the comma, or a full month name.
    for fmt in ["%b %d %Y", "%B %d %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(&s.trim().replace(',', ""), fmt) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}

/// `"1,240"` → 1240; `"2.0"` → 2.
pub fn int(s: &str) -> Option<i32> {
    let t: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    t.parse::<f64>().ok().map(|v| v.round() as i32)
}

pub fn decimal(s: &str) -> Option<f64> {
    let t: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    t.parse::<f64>().ok()
}

/// `"2/1.5"`, `"2 bd / 1.5 ba"`, `"Studio/1"` → (beds, baths).
pub fn bd_ba(s: &str) -> (Option<i32>, Option<f64>) {
    let t = s.to_lowercase();
    let mut parts = t.split(['/', '|', 'x']);
    let beds = parts.next().and_then(|b| {
        if b.contains("studio") {
            Some(0)
        } else {
            int(b)
        }
    });
    let baths = parts.next().and_then(decimal);
    (beds, baths)
}

/// `"Smith, John"` → `"John Smith"`. Several people stay as written, and so
/// does a company ("Acme, LLC").
pub fn person(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = t.to_lowercase();
    let company = ["llc", "inc", "corp", "trust", "lp", "ltd", "co."]
        .iter()
        .any(|w| {
            lower
                .split([' ', ',', '.'])
                .any(|x| x == w.trim_end_matches('.'))
        });
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    if !company
        && parts.len() == 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && !t.contains([';', '&'])
        && !lower.contains(" and ")
        && parts[0].split(' ').count() <= 2
    {
        return format!("{} {}", parts[1], parts[0]);
    }
    t
}

/// Whether a name reads as a company.
pub fn is_company(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        " llc",
        " l.l.c",
        " inc",
        " corp",
        " company",
        " trust",
        " lp",
        " ltd",
        " partners",
        " holdings",
        " properties",
        " group",
        " realty",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

/// The first email in a cell that may hold several.
pub fn email(s: &str) -> Option<String> {
    s.split([';', ',', ' ', '\n'])
        .map(|x| x.trim().trim_matches(['<', '>']).to_lowercase())
        .find(|x| x.contains('@') && x.contains('.') && !x.starts_with('@'))
}

/// A lease status from whatever the other tool called it: `upcoming`,
/// `active`, `notice`, `expired` or `ended`.
pub fn lease_status(s: Option<&str>, end: Option<&str>, today: &str) -> &'static str {
    let n = s.map(norm).unwrap_or_default();
    match n.as_str() {
        x if x.contains("notice") || x.contains("ntv") || x.contains("evict") => "notice",
        x if x.contains("future")
            || x.contains("upcoming")
            || x.contains("pending")
            || x.contains("applicant") =>
        {
            "upcoming"
        }
        x if x.contains("past")
            || x.contains("former")
            || x.contains("moved")
            || x.contains("ended")
            || x.contains("terminated") =>
        {
            "ended"
        }
        x if x.contains("vacant") => "ended",
        _ => match end {
            Some(e) if e < today && !n.contains("monthtomonth") && !n.contains("mtm") => "expired",
            _ => "active",
        },
    }
}

/// Trades from a cell like `"Plumbing; HVAC/Heating"`, kept to the ones the
/// service desk knows.
pub fn trades(s: &str) -> Vec<String> {
    let raw: Vec<String> = s
        .split([';', ',', '/', '|', '&'])
        .map(|t| {
            let n = norm(t);
            match n.as_str() {
                "heating" | "cooling" | "airconditioning" | "ac" | "heatingandcooling" => {
                    "hvac".into()
                }
                "electric" | "electrician" => "electrical".into(),
                "plumber" => "plumbing".into(),
                "painting" | "painter" => "paint".into(),
                "roofer" => "roofing".into(),
                "landscaper" | "lawn" | "lawncare" | "yard" => "landscaping".into(),
                "pestcontrol" | "exterminator" => "pest".into(),
                "janitorial" | "cleaner" | "turnovercleaning" => "cleaning".into(),
                "handyman" | "maintenance" | "generalcontractor" | "gc" => "general".into(),
                "appliances" | "appliancerepair" => "appliance".into(),
                "pressurewashing" | "powerwashing" | "windowwashing" => "exterior".into(),
                "carpet" | "floors" => "flooring".into(),
                _ => n,
            }
        })
        .collect();
    crate::servicedesk::clean_trades(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn appfolio_rent_roll_maps_and_is_recognised() {
        let headers = h(&[
            "Property",
            "Unit",
            "Tenant",
            "Status",
            "BD/BA",
            "Sqft",
            "Rent",
            "Deposit",
            "Lease From",
            "Lease To",
            "Move-in",
            "Past Due",
        ]);
        assert_eq!(detect_source(&headers), "appfolio");
        let m = auto_map("tenants", &headers);
        assert_eq!(m["tenant_name"], "Tenant");
        assert_eq!(m["lease_start"], "Lease From");
        assert_eq!(m["lease_end"], "Lease To");
        assert_eq!(m["balance"], "Past Due");
        assert_eq!(m["bd_ba"], "BD/BA");
        assert_eq!(m["rent"], "Rent");
    }

    #[test]
    fn other_sources_and_a_plain_sheet() {
        assert_eq!(
            detect_source(&h(&[
                "Lease ID",
                "Rental property",
                "Unit",
                "Tenants",
                "Rent"
            ])),
            "buildium"
        );
        assert_eq!(
            detect_source(&h(&[
                "Resident Code",
                "Resident",
                "Unit",
                "Lease Expiration"
            ])),
            "yardi"
        );
        assert_eq!(detect_source(&h(&["vantedge_id", "property"])), "vantedge");
        assert_eq!(detect_source(&h(&["Name", "Rent"])), "generic");
    }

    #[test]
    fn a_column_is_used_once() {
        // "Rent" is the tenant's rent; "Market Rent" doesn't take it too.
        let m = auto_map("tenants", &h(&["Tenant", "Market Rent", "Rent"]));
        assert_eq!(m["rent"], "Rent");
    }

    #[test]
    fn report_title_rows_and_totals_are_skipped() {
        let csv = "Rent Roll\nAs of 07/01/2024\n\nProperty,Unit,Tenant,Rent\nMaple Ct,1,\"Smith, John\",\"$1,250.00\"\nMaple Ct,2,Jane Doe,900\nTotal,,,\"2,150.00\"\n";
        let t = parse(csv).unwrap();
        assert_eq!(t.headers, h(&["Property", "Unit", "Tenant", "Rent"]));
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][2], "Smith, John");
    }

    #[test]
    fn windows_1252_files_read() {
        let bytes = b"Name,Notes\nCaf\xe9 Co,It\x92s fine\n";
        let t = parse(&decode(bytes)).unwrap();
        assert_eq!(t.rows[0][0], "Café Co");
        assert_eq!(t.rows[0][1], "It’s fine");
    }

    #[test]
    fn values_clean_up() {
        assert_eq!(money("$1,250.00"), Some(125_000));
        assert_eq!(money("(45.10)"), Some(-4_510));
        assert_eq!(money(""), None);
        assert_eq!(date("7/1/2024").as_deref(), Some("2024-07-01"));
        assert_eq!(date("07/01/24").as_deref(), Some("2024-07-01"));
        assert_eq!(date("2024-07-01T00:00:00").as_deref(), Some("2024-07-01"));
        assert_eq!(date("1-Jul-2024").as_deref(), Some("2024-07-01"));
        assert_eq!(date("Jul 1, 2024").as_deref(), Some("2024-07-01"));
        assert_eq!(date("soon"), None);
        assert_eq!(bd_ba("2/1.5"), (Some(2), Some(1.5)));
        assert_eq!(bd_ba("Studio/1"), (Some(0), Some(1.0)));
        assert_eq!(person("Smith, John"), "John Smith");
        assert_eq!(
            person("Smith, John; Smith, Jane"),
            "Smith, John; Smith, Jane"
        );
        assert_eq!(person("Acme, LLC"), "Acme, LLC");
        assert_eq!(email("a@x.com; b@y.com").as_deref(), Some("a@x.com"));
        assert!(is_company("Maple Holdings LLC"));
        assert!(!is_company("Jane Smith"));
    }

    #[test]
    fn statuses_and_trades() {
        let today = "2026-10-03";
        assert_eq!(lease_status(Some("Current"), None, today), "active");
        assert_eq!(lease_status(Some("Notice-Unrented"), None, today), "notice");
        assert_eq!(lease_status(Some("Future"), None, today), "upcoming");
        assert_eq!(lease_status(Some("Past"), None, today), "ended");
        assert_eq!(lease_status(None, Some("2025-01-31"), today), "expired");
        assert_eq!(
            lease_status(Some("Month-to-Month"), Some("2025-01-31"), today),
            "active"
        );
        assert_eq!(
            trades("Plumbing; Heating & Cooling"),
            vec!["plumbing", "hvac"]
        );
    }
}
