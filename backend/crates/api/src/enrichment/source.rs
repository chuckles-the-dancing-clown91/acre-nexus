//! The enrichment **source taxonomy**: the data sources the engine can fetch and
//! their mapping to background-job kinds. The orchestrator job fans out into one
//! child job per source so each runs (and retries) independently on the queue.

/// One automated data source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Live geocode (latitude/longitude) — the one real outbound integration.
    Geocode,
    /// Parcel / county record (APN, zoning, owner, last sale, physical attrs).
    Parcel,
    /// Tax assessment history.
    Tax,
    /// Automated valuation (AVM) + rent estimate.
    Valuation,
    /// Assigned / nearby schools.
    Schools,
    /// Utility providers + typical cost.
    Utilities,
    /// Crime statistics for the area (FBI Crime Data Explorer).
    Crime,
}

/// Orchestrator job kind: fans out into one child job per [`Source`].
pub const ORCHESTRATOR_KIND: &str = "enrich_property";

/// Every job kind this engine owns (orchestrator + one per source), for the
/// module manifest.
pub const JOB_KINDS: &[&str] = &[
    ORCHESTRATOR_KIND,
    crate::geo::SCAN_KIND,
    crate::geo::FETCH_KIND,
    "enrich_geocode",
    "enrich_parcel",
    "enrich_tax",
    "enrich_valuation",
    "enrich_schools",
    "enrich_utilities",
    "enrich_crime",
    super::refresh::REFRESH_KIND,
];

impl Source {
    /// Short, stable key stored on `enrichment_run.source`.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Geocode => "geocode",
            Source::Parcel => "parcel",
            Source::Tax => "tax",
            Source::Valuation => "valuation",
            Source::Schools => "schools",
            Source::Utilities => "utilities",
            Source::Crime => "crime",
        }
    }

    /// The background-job kind that runs this source.
    pub fn job_kind(self) -> &'static str {
        match self {
            Source::Geocode => "enrich_geocode",
            Source::Parcel => "enrich_parcel",
            Source::Tax => "enrich_tax",
            Source::Valuation => "enrich_valuation",
            Source::Schools => "enrich_schools",
            Source::Utilities => "enrich_utilities",
            Source::Crime => "enrich_crime",
        }
    }

    /// The provider name recorded on the run (`census_geocoder` is live; the rest
    /// are deterministic simulations behind the same interface).
    pub fn provider(self) -> &'static str {
        match self {
            Source::Geocode => "census_geocoder",
            Source::Crime => "fbi_cde",
            Source::Parcel | Source::Tax | Source::Valuation => "simulated",
            _ => "simulated",
        }
    }

    /// All sources, in the order the orchestrator schedules them (geocode first
    /// so downstream sources could use coordinates).
    pub fn all() -> [Source; 7] {
        [
            Source::Geocode,
            Source::Parcel,
            Source::Tax,
            Source::Valuation,
            Source::Schools,
            Source::Utilities,
            Source::Crime,
        ]
    }

    /// The sources a nightly refresh re-runs: the ones that come from public
    /// records and drift (not the geocode, which is stable once it resolved).
    pub fn refreshable() -> [Source; 4] {
        [
            Source::Parcel,
            Source::Tax,
            Source::Valuation,
            Source::Crime,
        ]
    }

    /// Parse a source key (as accepted in the enrich request).
    pub fn from_str(s: &str) -> Option<Source> {
        Source::all().into_iter().find(|src| src.as_str() == s)
    }

    /// Resolve the source a child job kind belongs to.
    pub fn from_job_kind(kind: &str) -> Option<Source> {
        Source::all().into_iter().find(|src| src.job_kind() == kind)
    }
}
