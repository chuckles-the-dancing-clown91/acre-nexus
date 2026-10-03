//! **Property-level reach**: which of a workspace's properties a person may see.
//!
//! Permissions say *what* someone may do (`property:read`, `maintenance:manage`);
//! reach says *where*. The company owner and back office see the whole company.
//! Field roles (property manager, leasing agent, maintenance) and owners
//! (landlords) see only the properties they are assigned to, directly or through
//! an LLC they are assigned to.
//!
//! Enforcement happens in two places:
//!
//! * **Centrally**, in [`gate`], run by [`crate::db::RequestDb`] before any data
//!   route. For a property-scoped person it allows only:
//!   - routes that carry no property data ([`NEUTRAL`]);
//!   - list routes whose handler filters by [`Access`] itself ([`SELF_FILTERED`]);
//!   - routes addressed by one property, ticket, lease or unit that is within
//!     reach (`/properties/<id>/…`, `/tickets/<id>/…`, …). Anything outside reach
//!     answers 404, the same as a property in another company.
//!
//!   Everything else answers 403. New routes are therefore closed to scoped
//!   people until they are made reach-aware, never open by accident.
//! * **In list handlers**, which take the [`Access`] guard and narrow their query
//!   with [`Access::property_ids`].

use crate::state::AppState;
use entity::prelude::{
    Assignment, Lease, MaintenancePlan, MaintenanceTicket, Membership, Property, TicketLine,
    TicketPart, TicketQuote, Unit,
};
use rocket::http::{Method, Status};
use rocket::request::{FromRequest, Outcome, Request};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};
use std::collections::BTreeSet;
use uuid::Uuid;

/// Personas whose reach is limited to their assigned properties.
pub const SCOPED_PERSONAS: &[&str] = &[
    "property_manager",
    "leasing_agent",
    "maintenance",
    "landlord",
];

/// How far someone's view of the workspace extends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// Every property in the workspace.
    Company,
    /// Only these properties.
    Properties(BTreeSet<Uuid>),
}

/// The caller's reach in the active workspace, as a request guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    pub reach: Reach,
}

impl Access {
    pub fn company() -> Access {
        Access {
            reach: Reach::Company,
        }
    }

    pub fn is_scoped(&self) -> bool {
        matches!(self.reach, Reach::Properties(_))
    }

    /// The properties in reach, or `None` for the whole company. Use it to
    /// narrow a query: `if let Some(ids) = access.property_ids() { q.filter(col.is_in(ids)) }`.
    pub fn property_ids(&self) -> Option<Vec<Uuid>> {
        match &self.reach {
            Reach::Company => None,
            Reach::Properties(ids) => Some(ids.iter().copied().collect()),
        }
    }

    pub fn sees(&self, property_id: Uuid) -> bool {
        match &self.reach {
            Reach::Company => true,
            Reach::Properties(ids) => ids.contains(&property_id),
        }
    }
}

/// Work out someone's reach in a workspace.
///
/// * Platform staff (support, impersonation) see the whole company.
/// * Someone holding any persona outside [`SCOPED_PERSONAS`] in this workspace
///   (company owner, back office) sees the whole company.
/// * Someone holding only scoped personas sees the properties assigned to them,
///   plus the properties of LLCs assigned to them.
/// * A principal with no membership row at all (API tokens, synthetic test
///   principals) is governed by its permissions alone.
pub async fn compute(
    db: &impl ConnectionTrait,
    user_id: Uuid,
    tenant_id: Uuid,
    is_staff: bool,
) -> Result<Access, DbErr> {
    if is_staff {
        return Ok(Access::company());
    }
    let memberships = Membership::find()
        .filter(entity::membership::Column::UserId.eq(user_id))
        .filter(entity::membership::Column::TenantId.eq(tenant_id))
        .filter(entity::membership::Column::Status.ne("suspended"))
        .all(db)
        .await?;
    if memberships.is_empty()
        || memberships
            .iter()
            .any(|m| !SCOPED_PERSONAS.contains(&m.profile_type.as_str()))
    {
        return Ok(Access::company());
    }
    let assignments = Assignment::find()
        .filter(entity::assignment::Column::TenantId.eq(tenant_id))
        .filter(entity::assignment::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let mut ids: BTreeSet<Uuid> = assignments
        .iter()
        .filter(|a| a.subject_type == crate::routes::assignments::SUBJECT_PROPERTY)
        .map(|a| a.subject_id)
        .collect();
    let llcs: Vec<Uuid> = assignments
        .iter()
        .filter(|a| a.subject_type == crate::routes::assignments::SUBJECT_ENTITY)
        .map(|a| a.subject_id)
        .collect();
    if !llcs.is_empty() {
        for p in Property::find()
            .filter(entity::property::Column::TenantId.eq(tenant_id))
            .filter(entity::property::Column::LlcId.is_in(llcs))
            .all(db)
            .await?
        {
            ids.insert(p.id);
        }
    }
    Ok(Access {
        reach: Reach::Properties(ids),
    })
}

/// The caller's reach, computed once per request from the bearer token.
/// `None` when there is no signed-in user or no active workspace.
pub async fn for_request(req: &Request<'_>) -> Option<Access> {
    req.local_cache_async(async {
        let state = req.rocket().state::<AppState>()?;
        let token = req
            .headers()
            .get_one("Authorization")?
            .strip_prefix("Bearer ")?;
        let claims = crate::auth::decode_access_token(&state.config, token)?;
        let tenant = claims.tid?;
        match compute(&state.db, claims.sub, tenant, claims.staff).await {
            Ok(a) => Some(a),
            Err(e) => {
                tracing::error!("reach lookup failed: {e}");
                // Fail closed: an empty reach shows nothing.
                Some(Access {
                    reach: Reach::Properties(BTreeSet::new()),
                })
            }
        }
    })
    .await
    .clone()
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Access {
    type Error = ();
    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(for_request(req).await.unwrap_or_else(Access::company))
    }
}

/// Route prefixes with no property data: sign-in, the caller's own profile and
/// notifications, the module list the shell reads.
const NEUTRAL: &[&str] = &[
    "/auth",
    "/me",
    "/my",
    "/notifications",
    "/push",
    "/modules",
    "/theme",
];

/// List routes whose handlers narrow their results by [`Access`].
const SELF_FILTERED: &[(Method, &str)] = &[
    (Method::Get, "/properties"),
    (Method::Get, "/portfolio/summary"),
    (Method::Get, "/portfolio/llcs"),
    (Method::Get, "/search"),
    (Method::Get, "/tickets"),
    (Method::Get, "/leases"),
    // The service desk: the kit catalog (company-wide, no property data), a
    // work order from a kit and a routine on a property (the handlers check
    // the property is in reach), and the schedule (narrowed by reach).
    (Method::Get, "/issue-templates"),
    (Method::Get, "/ticket-actions"),
    (Method::Get, "/ticket-techs"),
    (Method::Get, "/ticket-queue"),
    (Method::Post, "/issue-templates/<id>/generate"),
    (Method::Get, "/maintenance-plans"),
    (Method::Post, "/maintenance-plans"),
];

/// Property routes a scoped person may not use even on their own properties:
/// deciding who is assigned where, and deleting the property, stay with the
/// company.
fn company_only(method: Method, route: &str) -> bool {
    route.starts_with("/properties/<id>/assignments") && method != Method::Get
        || route == "/properties/<id>" && method == Method::Delete
}

/// How a route names the property it touches, when it does.
enum Target {
    Property,
    Ticket,
    Lease,
    Unit,
    /// Records that hang off a work order or a property.
    TicketPart,
    TicketLine,
    TicketQuote,
    Plan,
}

fn target(route: &str) -> Option<Target> {
    let first = route
        .trim_start_matches('/')
        .split('/')
        .take(2)
        .collect::<Vec<_>>();
    match first.as_slice() {
        ["properties", "<id>"] => Some(Target::Property),
        ["tickets", "<id>"] => Some(Target::Ticket),
        ["leases", "<id>"] => Some(Target::Lease),
        ["units", "<id>"] => Some(Target::Unit),
        ["parts", "<id>"] => Some(Target::TicketPart),
        ["ticket-lines", "<id>"] => Some(Target::TicketLine),
        ["ticket-quotes", "<id>"] => Some(Target::TicketQuote),
        ["maintenance-plans", "<id>"] => Some(Target::Plan),
        _ => None,
    }
}

/// What the central check decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Out of reach: answer as if it did not exist.
    NotFound,
    /// This area isn't open to property-scoped people.
    Forbidden,
}

/// The pure part of [`gate`]: may a scoped person use this route? Returns the
/// verdict, or which record to look up to decide.
fn classify(method: Method, route: &str) -> Result<Verdict, Target> {
    // Whole segments only: "/me" must not open "/members".
    if NEUTRAL
        .iter()
        .any(|p| route == *p || route.starts_with(&format!("{p}/")))
    {
        return Ok(Verdict::Allow);
    }
    if SELF_FILTERED
        .iter()
        .any(|(m, r)| *m == method && *r == route)
    {
        return Ok(Verdict::Allow);
    }
    if company_only(method, route) {
        return Ok(Verdict::Forbidden);
    }
    match target(route) {
        Some(t) => Err(t),
        None => Ok(Verdict::Forbidden),
    }
}

/// The property of the work order a child record belongs to.
async fn ticket_property(
    db: &sea_orm::DatabaseConnection,
    ticket: Result<Option<Uuid>, DbErr>,
) -> Result<Option<Uuid>, DbErr> {
    match ticket? {
        Some(t) => Ok(MaintenanceTicket::find_by_id(t)
            .one(db)
            .await?
            .map(|t| t.property_id)),
        None => Ok(None),
    }
}

/// The central check, run before every data route.
pub async fn gate(req: &Request<'_>) -> Verdict {
    let Some(access) = for_request(req).await else {
        return Verdict::Allow;
    };
    if !access.is_scoped() {
        return Verdict::Allow;
    }
    let Some(route) = req.route() else {
        return Verdict::Allow;
    };
    let target = match classify(req.method(), route.uri.path()) {
        Ok(v) => return v,
        Err(t) => t,
    };
    let Some(id) = req.routed_segment(1).and_then(|s| Uuid::parse_str(s).ok()) else {
        return Verdict::NotFound;
    };
    let Some(state) = req.rocket().state::<AppState>() else {
        return Verdict::Forbidden;
    };
    let property = match target {
        Target::Property => Ok(Some(id)),
        Target::Ticket => MaintenanceTicket::find_by_id(id)
            .one(&state.db)
            .await
            .map(|t| t.map(|t| t.property_id)),
        Target::Lease => Lease::find_by_id(id)
            .one(&state.db)
            .await
            .map(|l| l.map(|l| l.property_id)),
        Target::Unit => Unit::find_by_id(id)
            .one(&state.db)
            .await
            .map(|u| u.map(|u| u.property_id)),
        Target::TicketPart => {
            ticket_property(
                &state.db,
                TicketPart::find_by_id(id)
                    .one(&state.db)
                    .await
                    .map(|p| p.map(|p| p.ticket_id)),
            )
            .await
        }
        Target::TicketLine => {
            ticket_property(
                &state.db,
                TicketLine::find_by_id(id)
                    .one(&state.db)
                    .await
                    .map(|l| l.map(|l| l.ticket_id)),
            )
            .await
        }
        Target::TicketQuote => {
            ticket_property(
                &state.db,
                TicketQuote::find_by_id(id)
                    .one(&state.db)
                    .await
                    .map(|q| q.map(|q| q.ticket_id)),
            )
            .await
        }
        Target::Plan => MaintenancePlan::find_by_id(id)
            .one(&state.db)
            .await
            .map(|p| p.map(|p| p.property_id)),
    };
    match property {
        Ok(Some(pid)) if access.sees(pid) => Verdict::Allow,
        Ok(_) => Verdict::NotFound,
        Err(e) => {
            tracing::error!("reach check failed: {e}");
            Verdict::Forbidden
        }
    }
}

impl Verdict {
    /// The status to refuse with, if refused.
    pub fn refusal(&self) -> Option<Status> {
        match self {
            Verdict::Allow => None,
            Verdict::NotFound => Some(Status::NotFound),
            Verdict::Forbidden => Some(Status::Forbidden),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(m: Method, r: &str) -> &'static str {
        match classify(m, r) {
            Ok(Verdict::Allow) => "allow",
            Ok(Verdict::NotFound) => "notfound",
            Ok(Verdict::Forbidden) => "forbidden",
            Err(Target::Property) => "property",
            Err(Target::Ticket) => "ticket",
            Err(Target::Lease) => "lease",
            Err(Target::Unit) => "unit",
            Err(Target::TicketPart) => "part",
            Err(Target::TicketLine) => "line",
            Err(Target::TicketQuote) => "quote",
            Err(Target::Plan) => "plan",
        }
    }

    #[test]
    fn scoped_routes_are_classified() {
        assert_eq!(verdict(Method::Get, "/auth/me"), "allow");
        assert_eq!(verdict(Method::Get, "/notifications/unread_count"), "allow");
        assert_eq!(verdict(Method::Get, "/properties"), "allow");
        assert_eq!(
            verdict(Method::Post, "/properties"),
            "forbidden",
            "no new properties"
        );
        assert_eq!(verdict(Method::Get, "/properties/<id>"), "property");
        assert_eq!(verdict(Method::Get, "/properties/<id>/leases"), "property");
        assert_eq!(verdict(Method::Patch, "/properties/<id>"), "property");
        assert_eq!(verdict(Method::Delete, "/properties/<id>"), "forbidden");
        assert_eq!(
            verdict(Method::Get, "/properties/<id>/assignments"),
            "property"
        );
        assert_eq!(
            verdict(Method::Post, "/properties/<id>/assignments"),
            "forbidden"
        );
        assert_eq!(verdict(Method::Patch, "/tickets/<id>"), "ticket");
        assert_eq!(verdict(Method::Get, "/leases/<id>"), "lease");
        assert_eq!(verdict(Method::Get, "/units/<id>/history"), "unit");
        assert_eq!(
            verdict(Method::Get, "/payments"),
            "forbidden",
            "closed until filtered"
        );
        assert_eq!(verdict(Method::Get, "/finance/series"), "forbidden");
        assert_eq!(verdict(Method::Get, "/members"), "forbidden");
        assert_eq!(verdict(Method::Get, "/issue-templates"), "allow");
        assert_eq!(verdict(Method::Get, "/ticket-actions"), "allow");
        assert_eq!(
            verdict(Method::Post, "/tickets/<id>/actions"),
            "ticket",
            "a button on a work order is checked against its property"
        );
        assert_eq!(
            verdict(Method::Post, "/issue-templates"),
            "forbidden",
            "the catalog is the company's"
        );
        assert_eq!(verdict(Method::Patch, "/parts/<id>"), "part");
        assert_eq!(verdict(Method::Delete, "/ticket-lines/<id>"), "line");
        assert_eq!(
            verdict(Method::Post, "/ticket-quotes/<id>/approve"),
            "quote"
        );
        assert_eq!(verdict(Method::Patch, "/maintenance-plans/<id>"), "plan");
        assert_eq!(
            verdict(Method::Post, "/tickets/<id>/tasks/<task_id>/dispatch"),
            "ticket"
        );
    }

    #[test]
    fn reach() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let scoped = Access {
            reach: Reach::Properties([a].into_iter().collect()),
        };
        assert!(scoped.is_scoped());
        assert!(scoped.sees(a));
        assert!(!scoped.sees(b));
        assert_eq!(scoped.property_ids(), Some(vec![a]));
        assert!(Access::company().sees(b));
        assert_eq!(Access::company().property_ids(), None);
    }
}
