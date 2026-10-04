//! **Resident profile** — the landlord-facing facts about a person who rents:
//! work, emergency contact, household, pets and rental history, plus the
//! mobile ID card built from them. The resident keeps it current; property
//! managers can edit it too. It fills their applications, and a lease is
//! written from it (`leasedoc`).

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use entity::prelude::ResidentProfile;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, schemars::JsonSchema, Clone, Default, Debug, PartialEq)]
pub struct Pet {
    pub name: String,
    /// `dog` | `cat` | `bird` | `fish` | `other`
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub breed: Option<String>,
    #[serde(default)]
    pub weight_lb: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub age_years: Option<f64>,
    #[serde(default)]
    pub service_animal: bool,
    /// ISO date the rabies/vaccine record runs to.
    #[serde(default)]
    pub vaccinated_through: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema, Clone, Default, Debug, PartialEq)]
pub struct Occupant {
    pub name: String,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub age: Option<i32>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema, Clone, Default, Debug, PartialEq)]
pub struct PriorRental {
    pub address: String,
    #[serde(default)]
    pub landlord_name: Option<String>,
    #[serde(default)]
    pub landlord_phone: Option<String>,
    #[serde(default)]
    pub rent_cents: Option<i64>,
    /// ISO date
    #[serde(default)]
    pub from: Option<String>,
    /// ISO date
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub reason_for_leaving: Option<String>,
}

/// What the resident (or their property manager) edits.
#[derive(Serialize, Deserialize, schemars::JsonSchema, Clone, Default, Debug)]
pub struct Extras {
    #[serde(default)]
    pub employer: Option<String>,
    #[serde(default)]
    pub job_title: Option<String>,
    #[serde(default)]
    pub employer_phone: Option<String>,
    #[serde(default)]
    pub emergency_contact_name: Option<String>,
    #[serde(default)]
    pub emergency_contact_phone: Option<String>,
    #[serde(default)]
    pub emergency_contact_relation: Option<String>,
    #[serde(default)]
    pub occupants: Vec<Occupant>,
    #[serde(default)]
    pub pets: Vec<Pet>,
    #[serde(default)]
    pub prior_rentals: Vec<PriorRental>,
}

impl Extras {
    pub fn from_model(m: &entity::resident_profile::Model) -> Extras {
        Extras {
            employer: m.employer.clone(),
            job_title: m.job_title.clone(),
            employer_phone: m.employer_phone.clone(),
            emergency_contact_name: m.emergency_contact_name.clone(),
            emergency_contact_phone: m.emergency_contact_phone.clone(),
            emergency_contact_relation: m.emergency_contact_relation.clone(),
            occupants: serde_json::from_value(m.occupants.clone()).unwrap_or_default(),
            pets: serde_json::from_value(m.pets.clone()).unwrap_or_default(),
            prior_rentals: serde_json::from_value(m.prior_rentals.clone()).unwrap_or_default(),
        }
    }

    pub fn has_emergency_contact(&self) -> bool {
        self.emergency_contact_name
            .as_deref()
            .is_some_and(|n| !n.trim().is_empty())
    }
}

fn clean(s: &mut Option<String>, max: usize) -> ApiResult<()> {
    if let Some(v) = s {
        let t = v.trim().to_string();
        if t.chars().count() > max {
            return Err(ApiError::BadRequest(format!(
                "keep each entry under {max} characters"
            )));
        }
        *s = if t.is_empty() { None } else { Some(t) };
    }
    Ok(())
}

/// Trim and bound what came in, so a profile can't grow without limit.
pub fn validate(e: &mut Extras) -> ApiResult<()> {
    for f in [
        &mut e.employer,
        &mut e.job_title,
        &mut e.employer_phone,
        &mut e.emergency_contact_name,
        &mut e.emergency_contact_phone,
        &mut e.emergency_contact_relation,
    ] {
        clean(f, 200)?;
    }
    if e.pets.len() > 10 || e.occupants.len() > 12 || e.prior_rentals.len() > 20 {
        return Err(ApiError::BadRequest(
            "too many entries: 10 pets, 12 occupants, 20 prior rentals at most".into(),
        ));
    }
    for p in &mut e.pets {
        p.name = p.name.trim().to_string();
        if p.name.is_empty() {
            return Err(ApiError::BadRequest("every pet needs a name".into()));
        }
        p.kind = match p.kind.trim().to_lowercase().as_str() {
            "dog" | "cat" | "bird" | "fish" => p.kind.trim().to_lowercase(),
            _ => "other".into(),
        };
        if p.weight_lb.is_some_and(|w| !(0.0..=500.0).contains(&w)) {
            return Err(ApiError::BadRequest(
                "pet weight is between 0 and 500 lb".into(),
            ));
        }
        clean(&mut p.breed, 120)?;
        clean(&mut p.color, 80)?;
        clean(&mut p.notes, 500)?;
        clean(&mut p.vaccinated_through, 10)?;
    }
    for o in &mut e.occupants {
        o.name = o.name.trim().to_string();
        if o.name.is_empty() {
            return Err(ApiError::BadRequest("every occupant needs a name".into()));
        }
        clean(&mut o.relation, 80)?;
    }
    for r in &mut e.prior_rentals {
        r.address = r.address.trim().to_string();
        if r.address.is_empty() {
            return Err(ApiError::BadRequest(
                "every prior rental needs an address".into(),
            ));
        }
        for f in [
            &mut r.landlord_name,
            &mut r.landlord_phone,
            &mut r.from,
            &mut r.to,
            &mut r.reason_for_leaving,
        ] {
            clean(f, 300)?;
        }
    }
    Ok(())
}

/// One line for the lease and the pet-fee rule: "Biscuit (dog, Labrador, 60 lb)".
pub fn pet_line(p: &Pet) -> String {
    let mut bits = vec![p.kind.clone()];
    if let Some(b) = p.breed.as_deref().filter(|b| !b.is_empty()) {
        bits.push(b.to_string());
    }
    if let Some(w) = p.weight_lb {
        bits.push(format!("{} lb", w.round() as i64));
    }
    if p.service_animal {
        bits.push("service animal".into());
    }
    format!("{} ({})", p.name, bits.join(", "))
}

pub fn pet_details(pets: &[Pet]) -> Option<String> {
    if pets.is_empty() {
        None
    } else {
        Some(pets.iter().map(pet_line).collect::<Vec<_>>().join("; "))
    }
}

/// Save a person's extras. `staff_notes` is `Some` only when staff set it.
pub async fn save(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
    actor: Uuid,
    mut extras: Extras,
    staff_notes: Option<Option<String>>,
) -> ApiResult<entity::resident_profile::Model> {
    validate(&mut extras)?;
    let now = Utc::now();
    let json =
        |v: serde_json::Result<serde_json::Value>| v.map_err(|e| ApiError::Internal(e.into()));
    let existing = ResidentProfile::find_by_id(user_id).one(db).await?;
    let saved = match existing {
        Some(m) => {
            let mut am: entity::resident_profile::ActiveModel = m.into();
            am.employer = Set(extras.employer.clone());
            am.job_title = Set(extras.job_title.clone());
            am.employer_phone = Set(extras.employer_phone.clone());
            am.emergency_contact_name = Set(extras.emergency_contact_name.clone());
            am.emergency_contact_phone = Set(extras.emergency_contact_phone.clone());
            am.emergency_contact_relation = Set(extras.emergency_contact_relation.clone());
            am.occupants = Set(json(serde_json::to_value(&extras.occupants))?);
            am.pets = Set(json(serde_json::to_value(&extras.pets))?);
            am.prior_rentals = Set(json(serde_json::to_value(&extras.prior_rentals))?);
            if let Some(n) = staff_notes {
                am.staff_notes = Set(n);
            }
            am.updated_by = Set(Some(actor));
            am.updated_at = Set(now.into());
            am.update(db).await?
        }
        None => {
            entity::resident_profile::ActiveModel {
                user_id: Set(user_id),
                tenant_id: Set(tenant_id),
                employer: Set(extras.employer.clone()),
                job_title: Set(extras.job_title.clone()),
                employer_phone: Set(extras.employer_phone.clone()),
                emergency_contact_name: Set(extras.emergency_contact_name.clone()),
                emergency_contact_phone: Set(extras.emergency_contact_phone.clone()),
                emergency_contact_relation: Set(extras.emergency_contact_relation.clone()),
                occupants: Set(json(serde_json::to_value(&extras.occupants))?),
                pets: Set(json(serde_json::to_value(&extras.pets))?),
                prior_rentals: Set(json(serde_json::to_value(&extras.prior_rentals))?),
                staff_notes: Set(staff_notes.flatten()),
                updated_by: Set(Some(actor)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?
        }
    };
    Ok(saved)
}

/// The mobile ID card: who they are and where they live, at a glance.
#[derive(Serialize, schemars::JsonSchema)]
pub struct IdCard {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub photo_url: Option<String>,
    /// Shown on the card and in the QR code, e.g. `RES-1A2B-3C4D`.
    pub code: String,
    pub company: String,
    /// `active` | `none`
    pub standing: String,
    pub residence: Option<Residence>,
    pub resident_since: Option<String>,
    pub emergency_contact: Option<String>,
    pub pets: Vec<String>,
    pub screening_cleared: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct Residence {
    pub property: String,
    pub address: String,
    pub unit: Option<String>,
    pub lease_start: String,
    pub lease_end: Option<String>,
}

/// `RES-1A2B-3C4D`, stable for a person.
pub fn code_for(user_id: Uuid) -> String {
    let h = user_id.simple().to_string().to_uppercase();
    format!("RES-{}-{}", &h[..4], &h[4..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_stable_and_shaped() {
        let id = Uuid::parse_str("1a2b3c4d-0000-0000-0000-000000000000").unwrap();
        assert_eq!(code_for(id), "RES-1A2B-3C4D");
    }

    #[test]
    fn pets_read_as_one_line_each() {
        let p = Pet {
            name: "Biscuit".into(),
            kind: "dog".into(),
            breed: Some("Labrador".into()),
            weight_lb: Some(60.4),
            ..Default::default()
        };
        assert_eq!(pet_line(&p), "Biscuit (dog, Labrador, 60 lb)");
        assert_eq!(pet_details(&[]), None);
        assert_eq!(
            pet_details(&[p.clone(), p]).unwrap(),
            "Biscuit (dog, Labrador, 60 lb); Biscuit (dog, Labrador, 60 lb)"
        );
    }

    #[test]
    fn validation_bounds_and_cleans() {
        let mut e = Extras {
            employer: Some("  Acme  ".into()),
            pets: vec![Pet {
                name: "Rex".into(),
                kind: "lizard".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        validate(&mut e).unwrap();
        assert_eq!(e.employer.as_deref(), Some("Acme"));
        assert_eq!(e.pets[0].kind, "other");
        e.pets[0].name = " ".into();
        assert!(validate(&mut e).is_err());
        let mut many = Extras {
            occupants: vec![
                Occupant {
                    name: "A".into(),
                    ..Default::default()
                };
                13
            ],
            ..Default::default()
        };
        assert!(validate(&mut many).is_err());
    }
}
