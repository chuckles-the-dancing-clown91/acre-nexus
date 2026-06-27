//! Idempotent demo seed data mirroring the design prototype: two client tenants
//! (Northwind, Cascade), platform staff + client admins, system roles, LLCs,
//! properties, listings and themes. All demo users share the password
//! `password`.

use crate::auth::hash_password;
use crate::rbac::{PERMISSION_CATALOG, PROFILE_TYPES, SYSTEM_ROLES};
use chrono::Utc;
use entity::prelude::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, Set};
use serde_json::json;
use uuid::Uuid;

const DEMO_PASSWORD: &str = "password";

/// Seed the database. The permission/persona catalogs are ensured (idempotently)
/// on every boot so they stay current; the heavier demo data is only created
/// when the database is empty.
pub async fn run(db: &DatabaseConnection) -> anyhow::Result<()> {
    ensure_catalogs(db).await?;

    if Tenant::find().count(db).await? > 0 {
        tracing::info!("seed: tenants already present, skipping demo data");
        return Ok(());
    }
    tracing::info!("seed: populating demo data");

    // ---- system roles + permissions ----
    let mut role_ids = std::collections::HashMap::new();
    for sr in SYSTEM_ROLES {
        let rid = Uuid::new_v4();
        role_ids.insert(sr.key, rid);
        entity::role::ActiveModel {
            id: Set(rid),
            tenant_id: Set(None),
            scope: Set(sr.scope.into()),
            key: Set(sr.key.into()),
            name: Set(sr.name.into()),
            description: Set(sr.description.into()),
            is_system: Set(true),
        }
        .insert(db)
        .await?;
        for p in sr.permissions {
            entity::role_permission::ActiveModel {
                id: sea_orm::ActiveValue::NotSet,
                role_id: Set(rid),
                permission: Set(p.as_str().into()),
            }
            .insert(db)
            .await?;
        }
    }

    // ---- tenants ----
    let northwind = seed_tenant(db, "northwind", "Northwind Property Group", "growth").await?;
    let cascade = seed_tenant(db, "cascade", "Cascade Living LLC", "starter").await?;

    // ---- users, profiles, memberships ----
    let pw = hash_password(DEMO_PASSWORD)?;

    // Acre HQ (platform staff).
    let avery = seed_user(db, None, "avery@acrehq.com", "Avery Stone", &pw, true).await?;
    seed_membership(
        db,
        &role_ids,
        avery,
        "platform",
        None,
        "acre_admin",
        Some("Founder"),
    )
    .await?;
    seed_profile(db, avery, "Avery", "Stone").await?;

    let sam = seed_user(db, None, "sam@acrehq.com", "Sam Okafor", &pw, true).await?;
    seed_membership(
        db,
        &role_ids,
        sam,
        "platform",
        None,
        "acre_support",
        Some("Support Lead"),
    )
    .await?;

    // Northwind (client workspace) — owner, back-office, and a landlord.
    let jordan = seed_user(
        db,
        Some(northwind),
        "jordan@northwind.com",
        "Jordan Mills",
        &pw,
        false,
    )
    .await?;
    seed_membership(
        db,
        &role_ids,
        jordan,
        "tenant",
        Some(northwind),
        "tenant_owner",
        Some("Principal"),
    )
    .await?;
    seed_profile(db, jordan, "Jordan", "Mills").await?;

    let morgan = seed_user(
        db,
        Some(northwind),
        "morgan@northwind.com",
        "Morgan Lee",
        &pw,
        false,
    )
    .await?;
    seed_membership(
        db,
        &role_ids,
        morgan,
        "tenant",
        Some(northwind),
        "back_office",
        Some("Operations"),
    )
    .await?;

    let lee = seed_user(
        db,
        Some(northwind),
        "lee@northwind.com",
        "Lee Carter",
        &pw,
        false,
    )
    .await?;
    seed_membership(
        db,
        &role_ids,
        lee,
        "tenant",
        Some(northwind),
        "landlord",
        Some("Owner — Maple Holdings"),
    )
    .await?;
    seed_profile(db, lee, "Lee", "Carter").await?;

    // Cascade (client workspace) — owner.
    let priya = seed_user(
        db,
        Some(cascade),
        "priya@cascade.com",
        "Priya Rao",
        &pw,
        false,
    )
    .await?;
    seed_membership(
        db,
        &role_ids,
        priya,
        "tenant",
        Some(cascade),
        "tenant_owner",
        Some("Principal"),
    )
    .await?;

    // ---- themes ----
    seed_theme(db, northwind, "Northwind Property Group", "#F5451F").await?;
    seed_theme(db, cascade, "Cascade Living LLC", "#1C7C53").await?;

    // ---- Northwind LLCs + properties ----
    let maple = seed_llc(db, northwind, "Maple Holdings LLC", "12-3456789", "OR").await?;
    let harbor = seed_llc(db, northwind, "Harbor LLC", "98-7654321", "OR").await?;
    let elm = seed_llc(db, northwind, "Elm Equity LLC", "45-6789012", "OR").await?;
    let alder = seed_llc(db, northwind, "Alder LLC", "33-2211009", "OR").await?;

    let maple_court = seed_property(
        db,
        northwind,
        maple,
        "The Maple Court",
        "123 Maple Ct",
        "Portland, OR",
        8,
        8,
        1_480_000,
        "Stabilized",
        2016,
        "Dana K.",
    )
    .await?;
    seed_property(
        db,
        northwind,
        maple,
        "Birchwood Lofts",
        "88 Birch Ave",
        "Portland, OR",
        12,
        11,
        1_790_000,
        "Vacant",
        2019,
        "Dana K.",
    )
    .await?;
    seed_property(
        db,
        northwind,
        harbor,
        "Harbor View",
        "700 Harbor Dr",
        "Portland, OR",
        24,
        23,
        5_060_000,
        "Vacant",
        2014,
        "Marcus R.",
    )
    .await?;
    seed_property(
        db,
        northwind,
        alder,
        "The Aldercroft",
        "15 Alder St",
        "Portland, OR",
        6,
        6,
        777_000,
        "Stabilized",
        2011,
        "Marcus R.",
    )
    .await?;
    seed_property(
        db,
        northwind,
        elm,
        "Elmwood Residences",
        "230 Elm Blvd",
        "Lake Oswego, OR",
        10,
        9,
        2_227_500,
        "Vacant",
        2021,
        "Dana K.",
    )
    .await?;

    // ---- Cascade LLCs + properties ----
    let riverside = seed_llc(db, cascade, "Riverside Holdings LLC", "77-1230988", "WA").await?;
    let cnorth = seed_llc(db, cascade, "Cascade North LLC", "77-4567321", "WA").await?;
    let riverside_flats = seed_property(
        db,
        cascade,
        riverside,
        "Riverside Flats",
        "12 River Rd",
        "Seattle, WA",
        40,
        38,
        6_200_000,
        "Stabilized",
        2018,
        "Lena T.",
    )
    .await?;
    seed_property(
        db,
        cascade,
        cnorth,
        "Cascade North Apartments",
        "400 North Ave",
        "Bellevue, WA",
        60,
        57,
        9_600_000,
        "Vacant",
        2020,
        "Lena T.",
    )
    .await?;
    seed_property(
        db,
        cascade,
        cnorth,
        "Birch & Main",
        "88 Main St",
        "Tacoma, WA",
        24,
        24,
        4_100_000,
        "Stabilized",
        2015,
        "Omar D.",
    )
    .await?;

    // ---- Northwind public listings (the website slice) ----
    seed_listing(db, northwind, "The Maple Court", "123 Maple Ct", "Portland, OR", 2, 1, 880, 185_000, "Available", "Now", "A bright 2-bed in a quiet, tree-lined court — hardwood floors, in-unit laundry, and a dedicated parking space.").await?;
    seed_listing(
        db,
        northwind,
        "Birchwood Lofts 5C",
        "88 Birch Ave",
        "Portland, OR",
        1,
        1,
        640,
        149_500,
        "Available",
        "Jul 15",
        "Open-plan loft with exposed brick and oversized windows, steps from the Birch Ave shops.",
    )
    .await?;
    seed_listing(
        db,
        northwind,
        "Cedar Park Townhome",
        "42 Cedar Park",
        "Beaverton, OR",
        3,
        2,
        1420,
        265_000,
        "New",
        "Aug 1",
        "Spacious three-bedroom townhome with an attached garage and private back patio.",
    )
    .await?;
    seed_listing(
        db,
        northwind,
        "Harbor View 12A",
        "700 Harbor Dr",
        "Portland, OR",
        2,
        2,
        1050,
        220_000,
        "Available",
        "Now",
        "Corner unit with river views, floor-to-ceiling glass, and a chef's kitchen.",
    )
    .await?;
    seed_listing(
        db,
        northwind,
        "The Aldercroft Studio",
        "15 Alder St",
        "Portland, OR",
        0,
        1,
        520,
        129_500,
        "Available",
        "Now",
        "Efficient studio in a classic 1911 building with original detailing and modern updates.",
    )
    .await?;
    seed_listing(
        db,
        northwind,
        "Elmwood Residences 3B",
        "230 Elm Blvd",
        "Lake Oswego, OR",
        2,
        2,
        1180,
        247_500,
        "New",
        "Jul 1",
        "Brand-new construction with a balcony, smart-home package, and resort-style amenities.",
    )
    .await?;

    // ---- demo property intelligence (parcel/tax/valuation/schools/utilities) ----
    seed_intel(db, maple_court).await?;
    seed_intel(db, riverside_flats).await?;

    tracing::info!("seed: complete");
    Ok(())
}

async fn seed_tenant(
    db: &DatabaseConnection,
    slug: &str,
    name: &str,
    plan: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::tenant::ActiveModel {
        id: Set(id),
        slug: Set(slug.into()),
        name: Set(name.into()),
        plan: Set(plan.into()),
        status: Set("active".into()),
        custom_domain: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

async fn seed_user(
    db: &DatabaseConnection,
    tenant_id: Option<Uuid>,
    email: &str,
    name: &str,
    pw_hash: &str,
    staff: bool,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::user::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        email: Set(email.to_lowercase()),
        username: Set(None),
        password_hash: Set(pw_hash.into()),
        name: Set(name.into()),
        is_platform_staff: Set(staff),
        status: Set("active".into()),
        last_login_at: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// Idempotently ensure the permission and persona catalogs match the code. Safe
/// to run on every boot — inserts missing rows by primary key, leaves the rest.
async fn ensure_catalogs(db: &DatabaseConnection) -> anyhow::Result<()> {
    for p in PERMISSION_CATALOG {
        if entity::permission::Entity::find_by_id(p.key.to_string())
            .one(db)
            .await?
            .is_none()
        {
            entity::permission::ActiveModel {
                key: Set(p.key.into()),
                category: Set(p.category.into()),
                label: Set(p.label.into()),
                description: Set(p.description.into()),
                scope: Set(p.scope.into()),
                is_system: Set(true),
            }
            .insert(db)
            .await?;
        }
    }
    for t in PROFILE_TYPES {
        if entity::profile_type::Entity::find_by_id(t.key.to_string())
            .one(db)
            .await?
            .is_none()
        {
            entity::profile_type::ActiveModel {
                key: Set(t.key.into()),
                scope: Set(t.scope.into()),
                label: Set(t.label.into()),
                description: Set(t.description.into()),
                default_role: Set(t.default_role.into()),
                is_system: Set(true),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// Insert a membership and grant the persona's default system role.
async fn seed_membership(
    db: &DatabaseConnection,
    role_ids: &std::collections::HashMap<&'static str, Uuid>,
    user_id: Uuid,
    scope: &str,
    tenant_id: Option<Uuid>,
    persona: &str,
    title: Option<&str>,
) -> anyhow::Result<()> {
    entity::membership::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        scope: Set(scope.into()),
        tenant_id: Set(tenant_id),
        profile_type: Set(persona.into()),
        title: Set(title.map(|s| s.to_string())),
        status: Set("active".into()),
        is_primary: Set(true),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    if let Some(role_key) = crate::rbac::default_role_for_persona(persona) {
        if let Some(rid) = role_ids.get(role_key) {
            assign_role(db, user_id, *rid, tenant_id).await?;
        }
    }
    Ok(())
}

/// Seed a minimal demo profile (no sensitive PII).
async fn seed_profile(
    db: &DatabaseConnection,
    user_id: Uuid,
    first: &str,
    last: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::user_profile::ActiveModel {
        user_id: Set(user_id),
        legal_first_name: Set(Some(first.into())),
        legal_middle_name: Set(None),
        legal_last_name: Set(Some(last.into())),
        preferred_name: Set(Some(first.into())),
        date_of_birth: Set(None),
        phone: Set(None),
        address_line1: Set(None),
        address_line2: Set(None),
        city: Set(None),
        region: Set(None),
        postal_code: Set(None),
        country: Set(Some("US".into())),
        ssn_ciphertext: Set(None),
        ssn_nonce: Set(None),
        ssn_last4: Set(None),
        gov_id_type: Set(None),
        gov_id_ciphertext: Set(None),
        gov_id_nonce: Set(None),
        gov_id_last4: Set(None),
        photo_url: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn assign_role(
    db: &DatabaseConnection,
    user_id: Uuid,
    role_id: Uuid,
    tenant_id: Option<Uuid>,
) -> anyhow::Result<()> {
    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(user_id),
        role_id: Set(role_id),
        tenant_id: Set(tenant_id),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_theme(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    company: &str,
    accent: &str,
) -> anyhow::Result<()> {
    entity::theme::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        company_name: Set(company.into()),
        logo_url: Set(None),
        primary_color: Set(accent.into()),
        accent_color: Set(accent.into()),
        default_mode: Set("light".into()),
        legal_templates: Set(json!({
            "lease_intro": "This Residential Lease Agreement is entered into between {landlord} and {tenant}.",
            "late_fee": "A late fee of {late_fee} applies after a {grace_days}-day grace period.",
            "privacy": "We respect your privacy. Personal data is processed per our policy."
        })),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_llc(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    ein: &str,
    state: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::llc::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        name: Set(name.into()),
        ein: Set(ein.into()),
        state: Set(state.into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn seed_property(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    llc_id: Uuid,
    name: &str,
    address: &str,
    city: &str,
    units: i32,
    occupied: i32,
    rent_cents: i64,
    status: &str,
    year: i32,
    manager: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::property::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        llc_id: Set(Some(llc_id)),
        name: Set(name.into()),
        address: Set(address.into()),
        city: Set(city.into()),
        units: Set(units),
        occupied_units: Set(occupied),
        monthly_rent_cents: Set(rent_cents),
        status: Set(status.into()),
        year_built: Set(year),
        manager: Set(manager.into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// Populate a property's intelligence (parcel, tax, valuation, schools,
/// utilities) using the real enrichment engine's simulated providers, so the
/// detail page shows rich data out of the box. Geocode is skipped here to keep
/// `seed` offline; trigger it from the UI's "Enrich" action.
async fn seed_intel(db: &DatabaseConnection, property_id: Uuid) -> anyhow::Result<()> {
    use crate::enrichment::{runner, Source};
    if let Some(p) = Property::find_by_id(property_id).one(db).await? {
        for source in [
            Source::Parcel,
            Source::Tax,
            Source::Valuation,
            Source::Schools,
            Source::Utilities,
        ] {
            if let Err(e) = runner::run_source(db, &p, source).await {
                tracing::warn!("seed_intel {} failed: {e}", source.as_str());
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_listing(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    title: &str,
    address: &str,
    city: &str,
    beds: i32,
    baths: i32,
    sqft: i32,
    rent_cents: i64,
    status: &str,
    available_on: &str,
    description: &str,
) -> anyhow::Result<()> {
    entity::listing::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(None),
        title: Set(title.into()),
        address: Set(address.into()),
        city: Set(city.into()),
        beds: Set(beds),
        baths: Set(baths),
        sqft: Set(sqft),
        rent_cents: Set(rent_cents),
        status: Set(status.into()),
        available_on: Set(available_on.into()),
        description: Set(description.into()),
        is_public: Set(true),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}
