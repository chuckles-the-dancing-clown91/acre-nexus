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

    // Platform plane: staff roster (separate from any tenant membership).
    seed_platform_staff(db, avery).await?;
    seed_platform_staff(db, sam).await?;

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

    // ---- white-label domains (subdomain + a verified custom domain for Northwind) ----
    seed_domain(
        db,
        northwind,
        "northwind.acrenexus.com",
        "subdomain",
        "admin",
        true,
    )
    .await?;
    seed_domain(
        db,
        northwind,
        "owners.northwindpg.com",
        "custom",
        "owner",
        true,
    )
    .await?;
    seed_domain(
        db,
        northwind,
        "pay.northwindpg.com",
        "custom",
        "renter",
        false,
    )
    .await?;
    seed_domain(
        db,
        cascade,
        "cascade.acrenexus.com",
        "subdomain",
        "admin",
        true,
    )
    .await?;

    // ---- onboarding workflows (one per tenant) ----
    seed_onboarding(db, northwind, "live").await?;
    seed_onboarding(db, cascade, "portfolio_imported").await?;

    // ---- fee schedule (conditional fees / discounts / amenities) ----
    seed_fee(db, northwind, "pet_fee", "fee", "Pet rent", 5000, true, "has_pet",
        "Resident discloses pet(s): {pet_details}. A monthly pet rent of {amount} applies and resident agrees to the pet addendum.").await?;
    seed_fee(
        db,
        northwind,
        "military_discount",
        "discount",
        "Military discount",
        10000,
        true,
        "is_military",
        "A monthly military/veteran discount of {amount} is applied to base rent.",
    )
    .await?;
    seed_fee(db, northwind, "garage", "amenity", "Reserved garage", 15000, true, "manual",
        "Resident is assigned one reserved garage for vehicle: {vehicles}. Monthly amenity fee of {amount} applies.").await?;
    seed_fee(
        db,
        northwind,
        "application_fee",
        "fee",
        "Application fee",
        5000,
        false,
        "manual",
        "A one-time application/processing fee of {amount}.",
    )
    .await?;

    // ---- Northwind LLCs + properties ----
    let maple = seed_llc(db, northwind, "Maple Holdings LLC", "12-3456789", "OR").await?;
    let harbor = seed_llc(db, northwind, "Harbor LLC", "98-7654321", "OR").await?;
    let elm = seed_llc(db, northwind, "Elm Equity LLC", "45-6789012", "OR").await?;
    let alder = seed_llc(db, northwind, "Alder LLC", "33-2211009", "OR").await?;

    // ---- Maple Holdings cap table + bank accounts + a portfolio ----
    let firm_owner = seed_owner(db, northwind, "firm", "Northwind Property Group").await?;
    let investor = seed_owner(db, northwind, "individual", "Dana Kessler").await?;
    seed_entity_ownership(db, northwind, maple, firm_owner, 6000, "manager").await?;
    seed_entity_ownership(db, northwind, maple, investor, 4000, "investor").await?;
    seed_bank_account(
        db,
        northwind,
        maple,
        "operating",
        "First Cascade Bank",
        "1042",
    )
    .await?;
    seed_bank_account(db, northwind, maple, "trust", "First Cascade Bank", "7781").await?;
    let _flip_portfolio = seed_portfolio(db, northwind, "Pacific NW Cashflow", "cashflow").await?;

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

    // ---- demo entities (counterparties) + financing on Maple Court ----
    let bank = seed_counterparty(
        db,
        northwind,
        "lender",
        "First Cascade Bank",
        Some("Riley Chen, Loan Officer"),
        Some("(503) 555-0142"),
    )
    .await?;
    seed_counterparty_note(
        db,
        northwind,
        bank,
        "Pre-approved Northwind for portfolio refis at prime + 1.5%.",
    )
    .await?;
    seed_counterparty(
        db,
        northwind,
        "insurer",
        "Cascade Mutual Insurance",
        Some("Claims: (800) 555-0190"),
        None,
    )
    .await?;
    let contractor = seed_counterparty(
        db,
        northwind,
        "contractor",
        "Birch & Co. General Contracting",
        Some("Sam Ortiz"),
        Some("(503) 555-0177"),
    )
    .await?;
    // A 1st-lien mortgage on Maple Court through First Cascade Bank.
    seed_mortgage(db, northwind, maple_court, bank).await?;

    // ---- demo rentals: units, leases, a payment, a maintenance ticket ----
    let unit_a = seed_unit(db, northwind, maple_court, "1A", 2, 1.0, 185_000).await?;
    let unit_b = seed_unit(db, northwind, maple_court, "2B", 1, 1.0, 162_000).await?;
    // A current tenant and a behind tenant.
    seed_lease(
        db,
        northwind,
        maple_court,
        unit_a,
        "Taylor Brooks",
        "taylor@example.com",
        185_000,
        "2024-09-01",
        "active",
        "current",
        0,
    )
    .await?;
    let behind = seed_lease(
        db,
        northwind,
        maple_court,
        unit_b,
        "Jordan Avery",
        "jordan.a@example.com",
        162_000,
        "2024-06-15",
        "active",
        "late",
        162_000,
    )
    .await?;
    seed_lease_payment(db, northwind, behind, "2025-06-01", 162_000, "late").await?;
    // Demo vehicle + a garage amenity charge on the behind lease.
    seed_vehicle(
        db, northwind, behind, "Toyota", "Tacoma", 2021, "Silver", "ABC-1234",
    )
    .await?;
    seed_lease_charge(
        db,
        northwind,
        behind,
        "amenity",
        Some("garage"),
        "Reserved garage",
        15000,
        "manual",
        Some("Resident is assigned one reserved garage for vehicle: 2021 Toyota Tacoma (Silver, plate ABC-1234)."),
    )
    .await?;
    // An open work order assigned to the contractor.
    seed_ticket(
        db,
        northwind,
        maple_court,
        Some(unit_b),
        contractor,
        "Kitchen faucet leaking",
        "plumbing",
        "high",
        "in_progress",
    )
    .await?;

    // ---- demo title: ownership (deed) + liens ----
    seed_ownership(db, northwind, maple_court, maple, "Maple Holdings LLC").await?;
    seed_lien(
        db,
        northwind,
        maple_court,
        Some(bank),
        "First Cascade Bank",
        "mortgage",
        Some(115_000_000),
        Some(1),
        "active",
    )
    .await?;
    seed_lien(
        db,
        northwind,
        maple_court,
        None,
        "Multnomah County Tax Assessor",
        "tax",
        Some(420_000),
        Some(2),
        "active",
    )
    .await?;

    tracing::info!("seed: complete");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_unit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    unit_number: &str,
    beds: i32,
    baths: f64,
    rent_cents: i64,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::unit::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        unit_number: Set(unit_number.into()),
        beds: Set(Some(beds)),
        baths: Set(Some(baths)),
        sqft: Set(None),
        market_rent_cents: Set(Some(rent_cents)),
        status: Set("occupied".into()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn seed_lease(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    unit_id: Uuid,
    name: &str,
    email: &str,
    rent_cents: i64,
    start_date: &str,
    status: &str,
    payment_status: &str,
    balance_cents: i64,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::lease::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        unit_id: Set(Some(unit_id)),
        application_id: Set(None),
        tenant_name: Set(name.into()),
        tenant_email: Set(Some(email.into())),
        tenant_phone: Set(None),
        rent_cents: Set(rent_cents),
        deposit_cents: Set(Some(rent_cents)),
        start_date: Set(start_date.into()),
        end_date: Set(None),
        status: Set(status.into()),
        payment_status: Set(payment_status.into()),
        balance_cents: Set(balance_cents),
        has_pet: Set(false),
        pet_details: Set(None),
        is_military: Set(false),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn seed_fee(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    code: &str,
    kind: &str,
    label: &str,
    amount_cents: i64,
    recurring: bool,
    condition_type: &str,
    verbiage: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::fee_schedule::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        code: Set(code.into()),
        kind: Set(kind.into()),
        label: Set(label.into()),
        amount_cents: Set(amount_cents),
        recurring: Set(recurring),
        condition_type: Set(condition_type.into()),
        verbiage: Set(Some(verbiage.into())),
        active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_vehicle(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lease_id: Uuid,
    make: &str,
    model: &str,
    year: i32,
    color: &str,
    plate: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::vehicle::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        lease_id: Set(Some(lease_id)),
        application_id: Set(None),
        user_id: Set(None),
        make: Set(make.into()),
        model: Set(model.into()),
        year: Set(Some(year)),
        color: Set(Some(color.into())),
        license_plate: Set(Some(plate.into())),
        plate_state: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_lease_charge(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lease_id: Uuid,
    kind: &str,
    code: Option<&str>,
    label: &str,
    amount_cents: i64,
    source: &str,
    verbiage: Option<&str>,
) -> anyhow::Result<()> {
    entity::lease_charge::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        lease_id: Set(lease_id),
        kind: Set(kind.into()),
        code: Set(code.map(|s| s.to_string())),
        label: Set(label.into()),
        amount_cents: Set(amount_cents),
        recurring: Set(true),
        source: Set(source.into()),
        verbiage: Set(verbiage.map(|s| s.to_string())),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_lease_payment(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lease_id: Uuid,
    due_date: &str,
    amount_cents: i64,
    status: &str,
) -> anyhow::Result<()> {
    entity::lease_payment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        lease_id: Set(lease_id),
        due_date: Set(due_date.into()),
        amount_cents: Set(amount_cents),
        paid_date: Set(None),
        status: Set(status.into()),
        method: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_ticket(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    unit_id: Option<Uuid>,
    assignee_entity_id: Uuid,
    title: &str,
    category: &str,
    priority: &str,
    status: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::maintenance_ticket::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        unit_id: Set(unit_id),
        lease_id: Set(None),
        title: Set(title.into()),
        description: Set(None),
        category: Set(category.into()),
        priority: Set(priority.into()),
        status: Set(status.into()),
        assignee_user_id: Set(None),
        assignee_entity_id: Set(Some(assignee_entity_id)),
        reporter: Set(Some("Resident".into())),
        due_date: Set(None),
        cost_cents: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_ownership(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    llc_id: Uuid,
    owner_name: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::ownership::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        owner_kind: Set("llc".into()),
        owner_id: Set(Some(llc_id)),
        owner_name: Set(owner_name.into()),
        vesting: Set(Some("Sole ownership".into())),
        percent_bps: Set(10000),
        deed_type: Set(Some("Warranty".into())),
        deed_recorded_date: Set(Some("2021-04-15".into())),
        deed_reference: Set(Some("2021-048172".into())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn seed_lien(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    lienholder_id: Option<Uuid>,
    lienholder_name: &str,
    kind: &str,
    amount_cents: Option<i64>,
    position: Option<i32>,
    status: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::lien::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        lienholder_id: Set(lienholder_id),
        lienholder_name: Set(lienholder_name.into()),
        kind: Set(kind.into()),
        amount_cents: Set(amount_cents),
        position: Set(position),
        recorded_date: Set(Some("2021-04-15".into())),
        status: Set(status.into()),
        reference: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_counterparty(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: &str,
    name: &str,
    contact_name: Option<&str>,
    phone: Option<&str>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::counterparty::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        kind: Set(kind.into()),
        name: Set(name.into()),
        contact_name: Set(contact_name.map(|s| s.to_string())),
        email: Set(None),
        phone: Set(phone.map(|s| s.to_string())),
        website: Set(None),
        address: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

async fn seed_counterparty_note(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    counterparty_id: Uuid,
    body: &str,
) -> anyhow::Result<()> {
    entity::counterparty_note::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        counterparty_id: Set(counterparty_id),
        author_user_id: Set(None),
        body: Set(body.into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_mortgage(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    lender_id: Uuid,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::mortgage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        lender_id: Set(Some(lender_id)),
        kind: Set("purchase".into()),
        position: Set(1),
        original_amount_cents: Set(Some(120_000_000)),
        current_balance_cents: Set(Some(115_000_000)),
        interest_rate_bps: Set(Some(650)),
        term_months: Set(Some(360)),
        monthly_payment_cents: Set(Some(760_000)),
        escrow_monthly_cents: Set(Some(150_000)),
        start_date: Set(Some("2021-04-15".into())),
        maturity_date: Set(Some("2051-04-15".into())),
        loan_number: Set(Some("FCB-2021-0481".into())),
        status: Set("active".into()),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
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
        parent_org_id: Set(None),
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
        scope: Set(if tenant_id.is_some() {
            "tenant"
        } else {
            "platform"
        }
        .into()),
        scope_ref_id: Set(None),
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
        entity_type: Set("llc".into()),
        registered_agent: Set(None),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

async fn seed_platform_staff(db: &DatabaseConnection, user_id: Uuid) -> anyhow::Result<()> {
    entity::platform_staff::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(user_id),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_domain(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    hostname: &str,
    kind: &str,
    audience: &str,
    verified: bool,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::domain::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        hostname: Set(hostname.into()),
        kind: Set(kind.into()),
        audience: Set(audience.into()),
        verification_token: Set(if kind == "custom" && !verified {
            Some(format!("acre-verify={}", Uuid::new_v4().simple()))
        } else {
            None
        }),
        verified_at: Set(if verified { Some(now.into()) } else { None }),
        tls_status: Set(if verified { "active" } else { "pending" }.into()),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_onboarding(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    state: &str,
) -> anyhow::Result<()> {
    let now = Utc::now();
    entity::onboarding_workflow::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        state: Set(state.into()),
        steps: Set(json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_owner(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    kind: &str,
    name: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::owner::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        kind: Set(kind.into()),
        name: Set(name.into()),
        email: Set(None),
        phone: Set(None),
        notes: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

async fn seed_entity_ownership(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_id: Uuid,
    owner_id: Uuid,
    ownership_bps: i32,
    role: &str,
) -> anyhow::Result<()> {
    entity::entity_ownership::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        entity_id: Set(entity_id),
        owner_id: Set(owner_id),
        ownership_bps: Set(ownership_bps),
        role: Set(role.into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_bank_account(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_id: Uuid,
    kind: &str,
    institution: &str,
    last4: &str,
) -> anyhow::Result<()> {
    entity::bank_account::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        entity_id: Set(entity_id),
        kind: Set(kind.into()),
        institution: Set(institution.into()),
        masked_number: Set(Some(format!("••••{last4}"))),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn seed_portfolio(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    strategy: &str,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::portfolio::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        name: Set(name.into()),
        strategy: Set(strategy.into()),
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
        portfolio_id: Set(None),
        name: Set(name.into()),
        address: Set(address.into()),
        city: Set(city.into()),
        units: Set(units),
        occupied_units: Set(occupied),
        monthly_rent_cents: Set(rent_cents),
        status: Set(status.into()),
        year_built: Set(year),
        manager: Set(manager.into()),
        property_type: Set("multi_family".into()),
        strategy: Set("rental".into()),
        workflow_stage: Set("managing".into()),
        purchase_price_cents: Set(None),
        acquired_on: Set(None),
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
