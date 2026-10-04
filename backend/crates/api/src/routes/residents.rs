//! **Resident profile** routes.
//!
//! * `GET/PUT /my/resident` — the signed-in resident's own profile extras (work,
//!   emergency contact, household, pets, prior rentals), their **mobile ID card**
//!   and what "Apply now" will fill in. No staff permission needed.
//! * `GET /residents/profile?email=` and `PUT /residents/profile` — the same
//!   person as a property manager sees and edits them, with their tenancies and
//!   applications.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::resident::{self, Extras, IdCard, Pet, Residence};
use crate::routes::applications::dto::ApplicationResp;
use crate::routes::iam::dto::{ProfileDto, ProfileInput};
use crate::routes::iam::helpers::upsert_profile_inner;
use crate::routes::tenant_history::dto::TenancySummary;
use crate::routes::vehicles::dto::VehicleDto;
use crate::state::AppState;
use crate::tenancy::{Access, TenantScope};
use entity::prelude::{
    Application, Lease, Listing, Property, ResidentProfile, Theme, Unit, User, UserProfile, Vehicle,
};
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// What "Apply now" fills in from the profile, and what is still missing.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplyPrefill {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub current_address: Option<String>,
    pub annual_income_cents: Option<i64>,
    pub employer: Option<String>,
    pub job_title: Option<String>,
    pub is_military: bool,
    pub has_pet: bool,
    pub pets: Vec<Pet>,
    pub vehicles: usize,
    pub occupants: usize,
    pub prior_rentals: usize,
    pub emergency_contact: Option<String>,
    /// Short labels for what the resident has not filled in yet.
    pub missing: Vec<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MyResident {
    pub extras: Extras,
    pub card: IdCard,
    pub prefill: ApplyPrefill,
}

fn blank(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(|v| v.trim().is_empty())
}

/// The name on the card: the legal name when it is on file, else the account's.
fn display_name(me: &entity::user::Model, p: Option<&entity::user_profile::Model>) -> String {
    p.and_then(|p| match (&p.legal_first_name, &p.legal_last_name) {
        (Some(f), Some(l)) if !f.trim().is_empty() && !l.trim().is_empty() => {
            Some(format!("{} {}", f.trim(), l.trim()))
        }
        _ => None,
    })
    .unwrap_or_else(|| me.name.clone())
}

pub(crate) async fn build_card(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    me: &entity::user::Model,
    profile: Option<&entity::user_profile::Model>,
    extras: &Extras,
) -> ApiResult<IdCard> {
    let email = me.email.to_lowercase();
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .order_by_desc(entity::lease::Column::StartDate)
        .all(db)
        .await?;
    let mine: Vec<&entity::lease::Model> = leases
        .iter()
        .filter(|l| l.tenant_email.as_deref().map(str::to_lowercase).as_deref() == Some(&email))
        .collect();
    let active = mine.iter().find(|l| l.status == "active").copied();
    let residence = match active {
        Some(l) => {
            let prop = Property::find_by_id(l.property_id).one(db).await?;
            let unit = match l.unit_id {
                Some(u) => Unit::find_by_id(u).one(db).await?,
                None => None,
            };
            prop.map(|p| Residence {
                property: p.name,
                address: format!("{}, {}", p.address, p.city),
                unit: unit.map(|u| u.unit_number).filter(|n| n != "Home"),
                lease_start: l.start_date.clone(),
                lease_end: l.end_date.clone(),
            })
        }
        None => None,
    };
    let cleared = Application::find()
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .filter(entity::application::Column::Email.eq(email.clone()))
        .filter(entity::application::Column::ScreeningStatus.eq("cleared"))
        .one(db)
        .await?
        .is_some();
    let company = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|t| t.company_name)
        .unwrap_or_default();
    Ok(IdCard {
        name: display_name(me, profile),
        email: me.email.clone(),
        phone: profile.and_then(|p| p.phone.clone()),
        photo_url: profile.and_then(|p| p.photo_url.clone()),
        code: resident::code_for(me.id),
        company,
        standing: if active.is_some() { "active" } else { "none" }.into(),
        resident_since: mine.last().map(|l| l.start_date.clone()),
        residence,
        emergency_contact: extras.emergency_contact_name.as_ref().map(|n| {
            match extras.emergency_contact_phone.as_deref() {
                Some(p) if !p.is_empty() => format!("{n}, {p}"),
                _ => n.clone(),
            }
        }),
        pets: extras.pets.iter().map(resident::pet_line).collect(),
        screening_cleared: cleared,
    })
}

async fn build_prefill(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    me: &entity::user::Model,
    profile: Option<&entity::user_profile::Model>,
    extras: &Extras,
) -> ApiResult<ApplyPrefill> {
    let vehicles = Vehicle::find()
        .filter(entity::vehicle::Column::TenantId.eq(tenant_id))
        .filter(entity::vehicle::Column::UserId.eq(me.id))
        .all(db)
        .await?
        .len();
    let phone = profile.and_then(|p| p.phone.clone());
    let income = profile
        .and_then(|p| p.annual_income_cents)
        .filter(|c| *c > 0);
    let current_address = profile.and_then(|p| {
        let line = [&p.address_line1, &p.city, &p.region, &p.postal_code]
            .iter()
            .filter_map(|v| v.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .collect::<Vec<_>>()
            .join(", ");
        (!line.is_empty()).then_some(line)
    });
    let mut missing = Vec::new();
    if blank(&phone) {
        missing.push("Phone".to_string());
    }
    if current_address.is_none() {
        missing.push("Current address".into());
    }
    if profile.and_then(|p| p.date_of_birth).is_none() {
        missing.push("Date of birth".into());
    }
    if income.is_none() {
        missing.push("Income".into());
    }
    if blank(&extras.employer) {
        missing.push("Employer".into());
    }
    if !extras.has_emergency_contact() {
        missing.push("Emergency contact".into());
    }
    if extras.prior_rentals.is_empty() {
        missing.push("Rental history".into());
    }
    Ok(ApplyPrefill {
        name: display_name(me, profile),
        email: me.email.clone(),
        phone,
        current_address,
        annual_income_cents: income,
        employer: extras.employer.clone(),
        job_title: extras.job_title.clone(),
        is_military: profile.is_some_and(|p| p.is_military),
        has_pet: !extras.pets.is_empty() || profile.is_some_and(|p| p.has_pet),
        pets: extras.pets.clone(),
        vehicles,
        occupants: extras.occupants.len(),
        prior_rentals: extras.prior_rentals.len(),
        emergency_contact: extras.emergency_contact_name.clone(),
        missing,
    })
}

async fn my_resident_view(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<MyResident> {
    let me = User::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;
    let profile = UserProfile::find_by_id(user_id).one(db).await?;
    let extras = ResidentProfile::find_by_id(user_id)
        .one(db)
        .await?
        .map(|m| Extras::from_model(&m))
        .unwrap_or_default();
    Ok(MyResident {
        card: build_card(db, tenant_id, &me, profile.as_ref(), &extras).await?,
        prefill: build_prefill(db, tenant_id, &me, profile.as_ref(), &extras).await?,
        extras,
    })
}

/// Keep the account profile's pet flag and text in step with the pet list, so
/// the pet-fee rule and applications read the same thing.
async fn sync_pet_flags(
    db: &impl ConnectionTrait,
    pii_key: &[u8],
    user_id: Uuid,
    pets: &[Pet],
) -> ApiResult<()> {
    upsert_profile_inner(
        db,
        pii_key,
        user_id,
        &ProfileInput {
            has_pet: Some(!pets.is_empty()),
            pet_details: Some(resident::pet_details(pets).unwrap_or_default()),
            ..Default::default()
        },
    )
    .await
}

/// `GET /my/resident` — my profile extras, ID card and application prefill.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/resident")]
pub async fn my_resident(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<MyResident>> {
    Ok(Json(
        my_resident_view(&db, scope.tenant_id, user.user_id).await?,
    ))
}

/// `PUT /my/resident` — save my work, emergency contact, household, pets and
/// rental history.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[put("/my/resident", data = "<body>")]
pub async fn update_my_resident(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<Extras>,
) -> ApiResult<Json<MyResident>> {
    let extras = body.into_inner();
    let saved = resident::save(
        &db,
        scope.tenant_id,
        user.user_id,
        user.user_id,
        extras,
        None,
    )
    .await?;
    sync_pet_flags(
        &db,
        &state.config.pii_key,
        user.user_id,
        &Extras::from_model(&saved).pets,
    )
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROFILE_WRITE,
        Some("resident_profile"),
        Some(user.user_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "self_service": true, "area": "resident" })),
    )
    .await;
    Ok(Json(
        my_resident_view(&db, scope.tenant_id, user.user_id).await?,
    ))
}

// ---- staff ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct TenancyRow {
    #[serde(flatten)]
    pub tenancy: TenancySummary,
    pub unit_number: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ResidentDetail {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// The person's platform account, when they have one. Without it the profile
    /// can be read but not edited: invite them first.
    pub user_id: Option<Uuid>,
    pub profile: Option<ProfileDto>,
    pub extras: Extras,
    pub staff_notes: Option<String>,
    pub vehicles: Vec<VehicleDto>,
    pub tenancies: Vec<TenancyRow>,
    pub applications: Vec<ApplicationResp>,
    pub card: Option<IdCard>,
}

async fn detail_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    access: &Access,
    email: &str,
) -> ApiResult<ResidentDetail> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::BadRequest("email is required".into()));
    }
    let all_leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .order_by_desc(entity::lease::Column::StartDate)
        .all(db)
        .await?;
    let leases: Vec<_> = all_leases
        .into_iter()
        .filter(|l| l.tenant_email.as_deref().map(str::to_lowercase).as_deref() == Some(&email))
        .filter(|l| access.sees(l.property_id))
        .collect();

    // An application belongs to a property through its listing.
    let listing_props: HashMap<Uuid, Option<Uuid>> = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| (l.id, l.property_id))
        .collect();
    let apps: Vec<_> = Application::find()
        .filter(entity::application::Column::TenantId.eq(tenant_id))
        .filter(entity::application::Column::Email.eq(email.clone()))
        .order_by_desc(entity::application::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .filter(|a| {
            !access.is_scoped()
                || a.listing_id
                    .and_then(|l| listing_props.get(&l).copied().flatten())
                    .is_some_and(|p| access.sees(p))
        })
        .collect();
    if leases.is_empty() && apps.is_empty() {
        return Err(ApiError::NotFound("resident not found".into()));
    }

    let prop_names: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let unit_numbers: HashMap<Uuid, String> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.unit_number))
        .collect();

    let name = leases
        .first()
        .map(|l| l.tenant_name.clone())
        .or_else(|| apps.first().map(|a| a.applicant_name.clone()))
        .unwrap_or_default();
    let phone = leases
        .iter()
        .find_map(|l| l.tenant_phone.clone())
        .or_else(|| apps.iter().map(|a| a.phone.clone()).find(|p| !p.is_empty()));

    let account = User::find()
        .filter(entity::user::Column::Email.eq(email.clone()))
        .filter(entity::user::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let (profile, extras, staff_notes, vehicles, card) = match &account {
        Some(me) => {
            let p = UserProfile::find_by_id(me.id).one(db).await?;
            let rp = ResidentProfile::find_by_id(me.id).one(db).await?;
            let ex = rp.as_ref().map(Extras::from_model).unwrap_or_default();
            let v = Vehicle::find()
                .filter(entity::vehicle::Column::TenantId.eq(tenant_id))
                .filter(entity::vehicle::Column::UserId.eq(me.id))
                .all(db)
                .await?
                .into_iter()
                .map(VehicleDto::from)
                .collect();
            let card = build_card(db, tenant_id, me, p.as_ref(), &ex).await?;
            (
                p.map(ProfileDto::from),
                ex,
                rp.and_then(|r| r.staff_notes),
                v,
                Some(card),
            )
        }
        None => (None, Extras::default(), None, Vec::new(), None),
    };

    Ok(ResidentDetail {
        name,
        email,
        phone,
        user_id: account.map(|a| a.id),
        profile,
        extras,
        staff_notes,
        vehicles,
        tenancies: leases
            .into_iter()
            .map(|l| {
                let unit_number = l.unit_id.and_then(|u| unit_numbers.get(&u).cloned());
                TenancyRow {
                    tenancy: TenancySummary::from_lease(l, &prop_names),
                    unit_number,
                }
            })
            .collect(),
        applications: apps.into_iter().map(ApplicationResp::from).collect(),
        card,
    })
}

/// `GET /residents/profile?email=` — one resident as staff see them.
#[rocket_okapi::openapi(tag = "Tenant History")]
#[get("/residents/profile?<email>")]
pub async fn resident_profile(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    email: &str,
) -> ApiResult<Json<ResidentDetail>> {
    user.require(Permission::LeaseRead)?;
    Ok(Json(
        detail_for(&db, scope.tenant_id, &access, email).await?,
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateResident {
    pub email: String,
    /// Contact, address, income and military status (the account profile).
    #[serde(default)]
    pub profile: Option<ProfileInput>,
    #[serde(default)]
    pub extras: Option<Extras>,
    /// Staff only. An empty string clears it.
    #[serde(default)]
    pub staff_notes: Option<String>,
}

/// `PUT /residents/profile` — a property manager updates a resident's profile.
#[rocket_okapi::openapi(tag = "Tenant History")]
#[put("/residents/profile", data = "<body>")]
pub async fn update_resident(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    body: Json<UpdateResident>,
) -> ApiResult<Json<ResidentDetail>> {
    user.require(Permission::LeaseManage)?;
    let b = body.into_inner();
    let current = detail_for(&db, scope.tenant_id, &access, &b.email).await?;
    let Some(uid) = current.user_id else {
        return Err(ApiError::BadRequest(
            "this person has no account yet, so invite them first".into(),
        ));
    };
    if let Some(p) = &b.profile {
        upsert_profile_inner(&db, &state.config.pii_key, uid, p).await?;
    }
    if b.extras.is_some() || b.staff_notes.is_some() {
        let extras = b.extras.unwrap_or(current.extras);
        let notes = b
            .staff_notes
            .map(|n| Some(n.trim().to_string()).filter(|n| !n.is_empty()));
        let saved = resident::save(&db, scope.tenant_id, uid, user.user_id, extras, notes).await?;
        sync_pet_flags(
            &db,
            &state.config.pii_key,
            uid,
            &Extras::from_model(&saved).pets,
        )
        .await?;
    }
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PROFILE_WRITE,
        Some("resident_profile"),
        Some(uid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "by_staff": true })),
    )
    .await;
    Ok(Json(
        detail_for(&db, scope.tenant_id, &access, &b.email).await?,
    ))
}

// ---- my home: the agreement, utilities and equipment ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct MyAgreement {
    pub title: String,
    /// `sent` | `signed`
    pub status: String,
    pub generated_at: String,
    pub signed_by: Option<String>,
    pub signed_at: Option<String>,
    pub sections: Option<Vec<crate::leasedoc::Section>>,
    pub body: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MyEquipment {
    pub name: String,
    pub kind: String,
    pub make: Option<String>,
    pub warranty_expires: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MyHome {
    /// The lease agreement once the office has sent it; never a draft.
    pub agreement: Option<MyAgreement>,
    pub utilities: Vec<crate::utilities::UtilityTerm>,
    pub equipment: Vec<MyEquipment>,
}

/// `GET /my/home` — what the resident agreed to and what comes with the home.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/home")]
pub async fn my_home(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<MyHome>> {
    let lease = crate::payments::lease_for_user(&db, scope.tenant_id, user.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))?;
    let property = Property::find_by_id(lease.property_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let agreement = entity::prelude::LeaseDocument::find()
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_document::Column::LeaseId.eq(lease.id))
        .filter(entity::lease_document::Column::Purpose.eq("lease"))
        .filter(entity::lease_document::Column::Status.ne("draft"))
        .order_by_desc(entity::lease_document::Column::GeneratedAt)
        .one(&db)
        .await?
        .map(|d| MyAgreement {
            title: d.title,
            status: d.status,
            generated_at: d.generated_at.to_rfc3339(),
            signed_by: d.signed_by,
            signed_at: d.signed_at.map(|t| t.to_rfc3339()),
            sections: d.sections.and_then(|v| serde_json::from_value(v).ok()),
            body: d.body,
        });
    let utilities =
        crate::utilities::terms(&db, scope.tenant_id, lease.property_id, lease.unit_id).await?;
    let equipment =
        crate::routes::lease_docs::generate::equipment_for(&db, scope.tenant_id, &lease, &property)
            .await?
            .into_iter()
            .map(|a| MyEquipment {
                name: a.name,
                kind: a.kind,
                make: a.make,
                warranty_expires: a.warranty_expires,
            })
            .collect();
    Ok(Json(MyHome {
        agreement,
        utilities,
        equipment,
    }))
}
