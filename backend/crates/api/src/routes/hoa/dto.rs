use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- associations ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateAssociationReq {
    pub name: String,
    pub property_id: Option<Uuid>,
    pub dues_cents: Option<i64>,
    /// `monthly` | `quarterly` | `annual`.
    pub dues_frequency: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AssociationDto {
    pub id: Uuid,
    pub name: String,
    pub property_id: Option<Uuid>,
    pub dues_cents: i64,
    pub dues_frequency: String,
    pub status: String,
    pub member_count: i64,
}

// ---- members ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateMemberReq {
    pub name: String,
    pub unit_label: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MemberDto {
    pub id: Uuid,
    pub association_id: Uuid,
    pub name: String,
    pub unit_label: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: String,
}

impl From<entity::hoa_member::Model> for MemberDto {
    fn from(m: entity::hoa_member::Model) -> Self {
        MemberDto {
            id: m.id,
            association_id: m.association_id,
            name: m.name,
            unit_label: m.unit_label,
            email: m.email,
            phone: m.phone,
            status: m.status,
        }
    }
}

// ---- assessments ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateAssessmentReq {
    /// Assess a single member; omit to bill **every active member** the amount.
    pub member_id: Option<Uuid>,
    /// Amount per member in cents; defaults to the association's standard dues.
    pub amount_cents: Option<i64>,
    pub description: Option<String>,
    /// Billing-period label, e.g. "2026-07".
    pub period: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AssessmentDto {
    pub id: Uuid,
    pub member_id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    pub period: Option<String>,
    pub due_date: Option<String>,
    pub status: String,
}

impl From<entity::hoa_assessment::Model> for AssessmentDto {
    fn from(m: entity::hoa_assessment::Model) -> Self {
        AssessmentDto {
            id: m.id,
            member_id: m.member_id,
            description: m.description,
            amount_cents: m.amount_cents,
            period: m.period,
            due_date: m.due_date,
            status: m.status,
        }
    }
}

// ---- violations ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateViolationReq {
    pub member_id: Uuid,
    pub kind: String,
    pub description: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateViolationReq {
    /// `open` | `cured` | `fined` | `closed`.
    pub status: Option<String>,
    pub fine_cents: Option<i64>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ViolationDto {
    pub id: Uuid,
    pub member_id: Uuid,
    pub kind: String,
    pub description: String,
    pub status: String,
    pub fine_cents: i64,
}

impl From<entity::hoa_violation::Model> for ViolationDto {
    fn from(m: entity::hoa_violation::Model) -> Self {
        ViolationDto {
            id: m.id,
            member_id: m.member_id,
            kind: m.kind,
            description: m.description,
            status: m.status,
            fine_cents: m.fine_cents,
        }
    }
}

// ---- ARC requests ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateArcReq {
    pub member_id: Uuid,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DecideArcReq {
    /// `approved` | `denied` | `withdrawn`.
    pub decision: String,
    pub note: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ArcDto {
    pub id: Uuid,
    pub member_id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub decision_note: Option<String>,
}

impl From<entity::hoa_arc_request::Model> for ArcDto {
    fn from(m: entity::hoa_arc_request::Model) -> Self {
        ArcDto {
            id: m.id,
            member_id: m.member_id,
            title: m.title,
            description: m.description,
            status: m.status,
            decision_note: m.decision_note,
        }
    }
}
