//! **Standard PM reports** (roadmap Phase 8, issue #56): rent roll, T-12,
//! aging, and delinquency — the reports every property manager expects on day
//! one. Each has a JSON endpoint (for the console) and an **export** endpoint
//! that renders the same data to CSV or PDF, gated by `report:read`.
//!
//! Reports read across the shipped rentals + general-ledger data: the rent roll
//! from leases/units, T-12 from the ledger (reusing
//! [`crate::accounting::account_activity`]), and aging/delinquency from the
//! outstanding `lease_payment` receivables.

pub mod aging;
pub mod delinquency;
pub mod rent_roll;
pub mod t12;

use crate::error::{ApiError, ApiResult};
use chrono::NaiveDate;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder};

/// A downloadable report file (CSV or PDF) with its content type + filename.
pub struct ReportFile {
    bytes: Vec<u8>,
    content_type: String,
    filename: String,
}

impl<'r> Responder<'r, 'static> for ReportFile {
    fn respond_to(self, _req: &'r Request<'_>) -> response::Result<'static> {
        let ct = self
            .content_type
            .parse::<ContentType>()
            .unwrap_or(ContentType::Binary);
        response::Response::build()
            .status(Status::Ok)
            .header(ct)
            .raw_header(
                "Content-Disposition",
                format!(
                    "attachment; filename=\"{}\"",
                    self.filename.replace('"', "")
                ),
            )
            .sized_body(self.bytes.len(), std::io::Cursor::new(self.bytes))
            .ok()
    }
}

/// A generic tabular view of a report, used to render CSV / PDF exports.
pub struct ReportTable {
    pub title: String,
    pub subtitle: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Optional trailing totals row (rendered emphasised).
    pub totals: Option<Vec<String>>,
}

/// Quote a CSV cell when it contains a delimiter, quote, or newline.
fn csv_cell(s: &str) -> String {
    if s.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn to_csv(table: &ReportTable) -> String {
    let mut out = String::new();
    let line = |cells: &[String]| -> String {
        cells
            .iter()
            .map(|c| csv_cell(c))
            .collect::<Vec<_>>()
            .join(",")
    };
    out.push_str(&line(&table.headers));
    out.push('\n');
    for row in &table.rows {
        out.push_str(&line(row));
        out.push('\n');
    }
    if let Some(totals) = &table.totals {
        out.push_str(&line(totals));
        out.push('\n');
    }
    out
}

fn to_pdf(table: &ReportTable) -> Vec<u8> {
    let mut text = format!("{}\n", table.title);
    if let Some(sub) = &table.subtitle {
        text.push_str(sub);
        text.push('\n');
    }
    text.push('\n');
    text.push_str(&table.headers.join(" | "));
    text.push('\n');
    text.push_str(&"-".repeat(60));
    text.push('\n');
    for row in &table.rows {
        text.push_str(&row.join(" | "));
        text.push('\n');
    }
    if let Some(totals) = &table.totals {
        text.push_str(&"-".repeat(60));
        text.push('\n');
        text.push_str(&totals.join(" | "));
        text.push('\n');
    }
    crate::pdf::text_to_pdf(&text)
}

/// Render a report table to a downloadable file in the requested `format`
/// (`csv` | `pdf`).
pub fn export(table: &ReportTable, basename: &str, format: &str) -> ApiResult<ReportFile> {
    match format.to_lowercase().as_str() {
        "csv" => Ok(ReportFile {
            bytes: to_csv(table).into_bytes(),
            content_type: "text/csv".into(),
            filename: format!("{basename}.csv"),
        }),
        "pdf" => Ok(ReportFile {
            bytes: to_pdf(table),
            content_type: "application/pdf".into(),
            filename: format!("{basename}.pdf"),
        }),
        other => Err(ApiError::BadRequest(format!(
            "unsupported format: {other} (expected csv or pdf)"
        ))),
    }
}

/// Today, UTC.
pub fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// Days a `YYYY-MM-DD` due date is past `today` (negative if not yet due).
pub fn days_past_due(due: &str, today: NaiveDate) -> i64 {
    NaiveDate::parse_from_str(due, "%Y-%m-%d")
        .map(|d| (today - d).num_days())
        .unwrap_or(0)
}

/// Parse optional `property_id` / `portfolio_id` query strings into UUIDs.
pub fn parse_scope(
    property_id: Option<String>,
    portfolio_id: Option<String>,
) -> ApiResult<(Option<uuid::Uuid>, Option<uuid::Uuid>)> {
    let parse = |s: Option<String>, what: &str| -> ApiResult<Option<uuid::Uuid>> {
        match s.filter(|v| !v.is_empty()) {
            Some(v) => uuid::Uuid::parse_str(&v)
                .map(Some)
                .map_err(|_| ApiError::BadRequest(format!("invalid {what}"))),
            None => Ok(None),
        }
    };
    Ok((
        parse(property_id, "property_id")?,
        parse(portfolio_id, "portfolio_id")?,
    ))
}
