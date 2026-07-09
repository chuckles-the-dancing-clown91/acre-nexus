//! Idempotent demo seed data mirroring the design prototype: two client tenants
//! (Northwind, Cascade), platform staff + client admins, system roles, LLCs,
//! properties, listings and themes. All demo users share the password
//! `password`.

use crate::auth::hash_password;
use crate::rbac::{PERMISSION_CATALOG, PROFILE_TYPES, SYSTEM_ROLES};
use chrono::{Datelike, Utc};
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

    // Never populate demo tenants + shared-password (`password`) logins in
    // production, even against an empty database — that's a footgun if a prod
    // environment is ever misconfigured with AUTO_MIGRATE on (issue #23). The
    // structural catalogs above are safe/idempotent and still run.
    if crate::config::is_production() {
        tracing::info!("seed: production environment, skipping demo data");
        return Ok(());
    }

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
    let operating_acct = seed_bank_account(
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

    // Hero photos for the two showcased profiles (upper-left of the profile).
    seed_property_image(
        db,
        maple_court,
        "https://images.unsplash.com/photo-1560448204-e02f11c3d0e2?auto=format&fit=crop&w=1200&q=80",
    )
    .await?;
    seed_property_image(
        db,
        riverside_flats,
        "https://images.unsplash.com/photo-1545324418-cc1a3fa10c00?auto=format&fit=crop&w=1200&q=80",
    )
    .await?;

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
    let current = seed_lease(
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
    // Jordan's monthly amount is rent + the reserved-garage amenity below, and
    // last month's rent is sitting unpaid (the balance).
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
        177_000,
    )
    .await?;
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
    // A couple of resolved work orders so the maintenance history isn't empty.
    seed_ticket(
        db,
        northwind,
        maple_court,
        Some(unit_a),
        contractor,
        "Annual HVAC service",
        "hvac",
        "normal",
        "resolved",
    )
    .await?;
    seed_ticket(
        db,
        northwind,
        maple_court,
        None,
        contractor,
        "Repaint common stairwell",
        "general",
        "low",
        "closed",
    )
    .await?;

    // ---- demo documents on Maple Court (insurance / loan / title / lease) ----
    // The recorded deed and the original signed lease need wet-ink originals, so
    // they carry a physical storage location.
    seed_document(
        db,
        northwind,
        "property",
        maple_court,
        "hazard-insurance-policy-2024.pdf",
        "insurance",
        false,
        None,
    )
    .await?;
    seed_document(
        db,
        northwind,
        "property",
        maple_court,
        "first-cascade-loan-agreement.pdf",
        "loan",
        false,
        None,
    )
    .await?;
    seed_document(
        db,
        northwind,
        "property",
        maple_court,
        "warranty-deed-recorded-2021.pdf",
        "title",
        true,
        Some("Fireproof safe — Northwind HQ, Drawer 3"),
    )
    .await?;
    seed_document(
        db,
        northwind,
        "property",
        maple_court,
        "original-signed-lease-1A.pdf",
        "lease",
        true,
        Some("Lease binder — Northwind HQ, Cabinet A"),
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

    // ---- Phase 3 demo: books, payment history, methods, feeds, payouts ----
    // Renter portal login for the current resident (password `password`):
    // taylor@example.com pays rent from /account/payments.
    let taylor_user = seed_user(
        db,
        Some(northwind),
        "taylor@example.com",
        "Taylor Brooks",
        &pw,
        false,
    )
    .await?;
    seed_membership(
        db,
        &role_ids,
        taylor_user,
        "tenant",
        Some(northwind),
        "renter",
        Some("Resident — Maple Court 1A"),
    )
    .await?;
    seed_profile(db, taylor_user, "Taylor", "Brooks").await?;

    // ---- Full maintenance demo: the equipment registry ----
    // Registered serviceable equipment on Maple Court; manuals/photos ride
    // the document service (owner_type "asset").
    let now = Utc::now();
    let ac_unit = Uuid::new_v4();
    for (id, unit, kind, name, make, model, warranty) in [
        (
            ac_unit,
            Some(unit_a),
            "hvac",
            "AC — Unit 1A living room",
            "Carrier",
            "Comfort 24ABC6",
            Some("2027-05-01"),
        ),
        (
            Uuid::new_v4(),
            None,
            "plumbing",
            "Water heater — basement",
            "Rheem",
            "XG40T06EC36U1",
            None,
        ),
        (
            Uuid::new_v4(),
            Some(unit_b),
            "appliance",
            "Refrigerator — Unit 2B",
            "Whirlpool",
            "WRF535SWHZ",
            Some("2026-11-15"),
        ),
    ] {
        entity::asset::ActiveModel {
            id: Set(id),
            tenant_id: Set(northwind),
            property_id: Set(maple_court),
            unit_id: Set(unit),
            kind: Set(kind.into()),
            name: Set(name.into()),
            make: Set(Some(make.into())),
            model: Set(Some(model.into())),
            serial_number: Set(None),
            install_date: Set(Some("2024-05-01".into())),
            warranty_expires: Set(warranty.map(str::to_string)),
            notes: Set(None),
            status: Set("active".into()),
            created_by: Set(Some(jordan)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    // The stockroom: common parts, one serialized item, one already at its
    // reorder level so the low-stock alert demos.
    for (name, sku, category, qty, unit_cost, reorder, serials) in [
        (
            "HVAC filter 20x25x1 (MERV 11)",
            Some("FLT-2025-11"),
            "part",
            24,
            Some(1_200i64),
            6,
            Vec::<&str>::new(),
        ),
        (
            "Kitchen faucet cartridge",
            Some("MOEN-1225"),
            "part",
            2,
            Some(2_400),
            3,
            vec![],
        ),
        (
            "Water heater element 4500W",
            Some("WH-4500"),
            "part",
            2,
            Some(3_500),
            0,
            vec!["WH45-00117", "WH45-00118"],
        ),
    ] {
        entity::inventory_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(northwind),
            property_id: Set(None),
            name: Set(name.into()),
            sku: Set(sku.map(str::to_string)),
            category: Set(category.into()),
            quantity: Set(qty),
            unit_cost_cents: Set(unit_cost),
            reorder_level: Set(reorder),
            storage_location: Set(Some("Shop — shelf B".into())),
            serial_numbers: Set(json!(serials)),
            notes: Set(None),
            low_stock_alerted_at: Set(None),
            status: Set("active".into()),
            created_by: Set(Some(jordan)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    // ---- Phase 5 demo: resident request, messaging, move-in inspection ----
    // A resident-reported maintenance request from the portal, in triage.
    let demo_ticket = Uuid::new_v4();
    entity::maintenance_ticket::ActiveModel {
        id: Set(demo_ticket),
        tenant_id: Set(northwind),
        property_id: Set(maple_court),
        unit_id: Set(Some(unit_a)),
        lease_id: Set(Some(current)),
        title: Set("Bedroom window won't latch".into()),
        description: Set(Some(
            "The latch on the bedroom window doesn't catch — it stays closed but won't lock."
                .into(),
        )),
        category: Set("general".into()),
        priority: Set("normal".into()),
        status: Set("triage".into()),
        assignee_user_id: Set(None),
        assignee_entity_id: Set(None),
        reporter: Set(Some("Taylor Brooks".into())),
        location: Set(Some("Bedroom".into())),
        access_notes: Set(Some("Weekdays after 5pm, or use the lockbox.".into())),
        permission_to_enter: Set(true),
        asset_id: Set(None),
        waiting_on: Set(None),
        follow_up_date: Set(None),
        rating: Set(None),
        review_comment: Set(None),
        reviewed_at: Set(None),
        due_date: Set(None),
        cost_cents: Set(None),
        first_response_at: Set(Some(now.into())),
        resolved_at: Set(None),
        sla_response_due_at: Set(Some((now + chrono::Duration::hours(24)).into())),
        sla_resolve_due_at: Set(Some((now + chrono::Duration::hours(168)).into())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    // The visibility split on its timeline: a public staff reply the
    // resident sees, and an internal note they don't.
    entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(northwind),
        ticket_id: Set(demo_ticket),
        author_user_id: Set(Some(jordan)),
        kind: Set("comment".into()),
        visibility: Set("public".into()),
        author_name: Set(Some("Jordan Mills".into())),
        body: Set(
            "Thanks Taylor — we'll have someone look at the latch this week. \
                   The lockbox works if you're out."
                .into(),
        ),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(northwind),
        ticket_id: Set(demo_ticket),
        author_user_id: Set(Some(jordan)),
        kind: Set("comment".into()),
        visibility: Set("internal".into()),
        author_name: Set(Some("Jordan Mills".into())),
        body: Set(
            "Note: same latch failed in 2B last year — if Birch confirms it's \
                   the same hardware batch, quote replacing all of them."
                .into(),
        ),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // A resident ↔ manager conversation with a staff reply.
    let thread_id = Uuid::new_v4();
    entity::message_thread::ActiveModel {
        id: Set(thread_id),
        tenant_id: Set(northwind),
        lease_id: Set(current),
        property_id: Set(maple_court),
        subject: Set("Package room access".into()),
        status: Set("open".into()),
        created_by: Set(taylor_user),
        last_message_at: Set(now.into()),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    entity::message::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(northwind),
        thread_id: Set(thread_id),
        sender_user_id: Set(taylor_user),
        sender_kind: Set("resident".into()),
        sender_name: Set("Taylor Brooks".into()),
        body: Set("Hi — my fob stopped opening the package room this week. \
                   Could you take a look?"
            .into()),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    entity::message::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(northwind),
        thread_id: Set(thread_id),
        sender_user_id: Set(jordan),
        sender_kind: Set("staff".into()),
        sender_name: Set("Jordan Mills".into()),
        body: Set(
            "Thanks for flagging it, Taylor — we've reset your fob's access. \
                   Give it a try and reply here if it still won't scan."
                .into(),
        ),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    // A completed move-in inspection on Taylor's lease with a few rated rows.
    let inspection_id = Uuid::new_v4();
    entity::inspection::ActiveModel {
        id: Set(inspection_id),
        tenant_id: Set(northwind),
        lease_id: Set(current),
        property_id: Set(maple_court),
        unit_id: Set(Some(unit_a)),
        kind: Set("move_in".into()),
        status: Set("completed".into()),
        scheduled_date: Set(Some("2024-08-30".into())),
        completed_at: Set(Some(now.into())),
        completed_by: Set(Some(jordan)),
        notes: Set(Some(
            "Unit in good shape at move-in; minor carpet wear noted in the bedroom.".into(),
        )),
        created_by: Set(Some(jordan)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    for (idx, (area, item, condition, notes)) in [
        (
            "Entry & living areas",
            "Doors, locks & hardware",
            "good",
            None,
        ),
        (
            "Entry & living areas",
            "Walls, ceiling & trim",
            "good",
            None,
        ),
        (
            "Kitchen",
            "Appliances (range, fridge, dishwasher)",
            "good",
            None,
        ),
        (
            "Bedrooms",
            "Flooring / carpet",
            "fair",
            Some("Light wear near the closet."),
        ),
        ("Systems & safety", "Smoke / CO detectors", "good", None),
    ]
    .into_iter()
    .enumerate()
    {
        entity::inspection_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(northwind),
            inspection_id: Set(inspection_id),
            area: Set(area.into()),
            item: Set(item.into()),
            condition: Set(condition.into()),
            notes: Set(notes.map(str::to_string)),
            sort_order: Set(idx as i32),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    // Maple Holdings' books: seed the default chart of accounts.
    crate::accounting::ensure_chart(db, northwind, maple).await?;

    // Saved payment methods: Taylor autopays with a visa; Jordan's card is the
    // canonical declining test number (…0002) so the failure path demos.
    seed_payment_method(
        db,
        northwind,
        current,
        Some(taylor_user),
        "card",
        Some("Visa"),
        "4242",
        true,
        Some(1),
    )
    .await?;
    seed_payment_method(
        db,
        northwind,
        behind,
        None,
        "card",
        Some("Visa"),
        "0002",
        false,
        None,
    )
    .await?;

    // Security deposits collected at move-in and held in trust — escrow cash
    // equals the deposit liability, so the trust ledger reconciles to zero.
    let dep = seed_lease_payment(
        db,
        northwind,
        current,
        "deposit",
        "2024-09-01",
        185_000,
        "paid",
        Some("2024-09-01"),
        Some("ach"),
    )
    .await?;
    crate::accounting::post_payment_settled(
        db,
        northwind,
        maple,
        Some(maple_court),
        current,
        "2024-09-01",
        185_000,
        "deposit",
        dep,
    )
    .await?;
    let dep = seed_lease_payment(
        db,
        northwind,
        behind,
        "deposit",
        "2024-06-15",
        162_000,
        "paid",
        Some("2024-06-15"),
        Some("ach"),
    )
    .await?;
    crate::accounting::post_payment_settled(
        db,
        northwind,
        maple,
        Some(maple_court),
        behind,
        "2024-06-15",
        162_000,
        "deposit",
        dep,
    )
    .await?;

    // Trailing 11 months of rent history with balanced ledger postings:
    // Taylor pays on time every month; Jordan pays until last month, which is
    // the unpaid balance carried on the lease above.
    let today = Utc::now().date_naive();
    let expenses_acct = crate::accounting::account(
        db,
        northwind,
        maple,
        crate::accounting::subtypes::PROPERTY_EXPENSES,
    )
    .await?;
    let operating_gl = crate::accounting::account(
        db,
        northwind,
        maple,
        crate::accounting::subtypes::OPERATING_BANK,
    )
    .await?;
    for i in (1..=11u32).rev() {
        let month = today
            .checked_sub_months(chrono::Months::new(i))
            .unwrap_or(today);
        let day = |d: u32| {
            chrono::NaiveDate::from_ymd_opt(month.year(), month.month(), d)
                .unwrap_or(month)
                .to_string()
        };
        let due = day(1);

        // Taylor: $1,850, settled on the 2nd by ACH.
        let paid = day(2);
        let p = seed_lease_payment(
            db,
            northwind,
            current,
            "rent",
            &due,
            185_000,
            "paid",
            Some(&paid),
            Some("ach"),
        )
        .await?;
        crate::accounting::post_rent_due(
            db,
            northwind,
            maple,
            Some(maple_court),
            current,
            &due,
            185_000,
            p,
        )
        .await?;
        crate::accounting::post_payment_settled(
            db,
            northwind,
            maple,
            Some(maple_court),
            current,
            &paid,
            185_000,
            "rent",
            p,
        )
        .await?;

        // Jordan: $1,770 (rent + garage); the most recent month goes unpaid.
        if i == 1 {
            let p = seed_lease_payment(
                db, northwind, behind, "rent", &due, 177_000, "late", None, None,
            )
            .await?;
            crate::accounting::post_rent_due(
                db,
                northwind,
                maple,
                Some(maple_court),
                behind,
                &due,
                177_000,
                p,
            )
            .await?;
        } else {
            let paid = day(5);
            let p = seed_lease_payment(
                db,
                northwind,
                behind,
                "rent",
                &due,
                177_000,
                "paid",
                Some(&paid),
                Some("card"),
            )
            .await?;
            crate::accounting::post_rent_due(
                db,
                northwind,
                maple,
                Some(maple_court),
                behind,
                &due,
                177_000,
                p,
            )
            .await?;
            crate::accounting::post_payment_settled(
                db,
                northwind,
                maple,
                Some(maple_court),
                behind,
                &paid,
                177_000,
                "rent",
                p,
            )
            .await?;
        }

        // Monthly operating spend keeps NOI (and payout math) honest.
        let spent = day(15);
        crate::accounting::post(
            db,
            northwind,
            crate::accounting::PostArgs {
                entity_id: maple,
                txn_date: &spent,
                memo: "Maintenance & operations",
                source_type: "manual",
                source_id: None,
                posted_by: None,
            },
            vec![
                crate::accounting::Leg::debit(expenses_acct.id, 120_000)
                    .on(Some(maple_court), None),
                crate::accounting::Leg::credit(operating_gl.id, 120_000),
            ],
        )
        .await?;
    }

    // Monthly snapshots so the dashboards chart occupancy / value history
    // (flow metrics per month come from the ledger + payments just seeded).
    let live = crate::billing::compute_point_in_time(db, northwind).await?;
    for i in (0..=11u32).rev() {
        let month = today
            .checked_sub_months(chrono::Months::new(i))
            .unwrap_or(today);
        let key = month.format("%Y-%m").to_string();
        let (rent_due, rent_collected) =
            crate::billing::month_rent_figures(db, northwind, &key).await?;
        let noi = crate::finance::month_noi(db, northwind, &key).await?;
        // A gentle upward drift in value; occupancy wobbles further back.
        let value = live.portfolio_value_cents - (i as i64) * (live.portfolio_value_cents / 200);
        let occupancy = (live.occupancy_bps - (i as i32 % 3) * 250).max(0);
        let delinquency = if i == 0 {
            live.delinquency_bps
        } else if i % 5 == 4 {
            5_000
        } else {
            0
        };
        entity::financial_snapshot::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(northwind),
            month: Set(key),
            occupancy_bps: Set(occupancy),
            delinquency_bps: Set(delinquency),
            portfolio_value_cents: Set(value),
            rent_due_cents: Set(rent_due),
            rent_collected_cents: Set(rent_collected),
            noi_cents: Set(noi),
            active_leases: Set(live.active_leases),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await?;
    }

    // Link the operating account for bank feeds (simulated Plaid). The first
    // billing cycle pulls the feed and auto-matches recent settled payments.
    let mut am: entity::bank_account::ActiveModel = BankAccount::find_by_id(operating_acct)
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("seeded bank account missing"))?
        .into();
    am.provider = Set(Some("plaid".into()));
    am.external_id = Set(Some(format!("sim_acct_{}", operating_acct.simple())));
    am.update(db).await?;

    // Owner payouts for Maple Holdings: two months ago executed + settled
    // (ledger entry + statement), last month left as a draft for the demo to
    // execute from the console.
    let two_ago = today
        .checked_sub_months(chrono::Months::new(2))
        .unwrap_or(today);
    let (ps, pe) = month_bounds(two_ago);
    let payout = crate::payouts::compute_payout(db, northwind, maple, &ps, &pe, None).await?;
    let mut am: entity::owner_payout::ActiveModel = payout.clone().into();
    am.status = Set("processing".into());
    am.provider = Set(Some("stripe".into()));
    am.external_id = Set(Some(format!("sim_po_{}", payout.id.simple())));
    am.update(db).await?;
    crate::payouts::settle_payout(db, northwind, payout.id, true, None).await;

    let last_month = today
        .checked_sub_months(chrono::Months::new(1))
        .unwrap_or(today);
    let (ps, pe) = month_bounds(last_month);
    crate::payouts::compute_payout(db, northwind, maple, &ps, &pe, None).await?;

    // ---- Acquisitions & Flips: a few demo deals for the pipeline board ----
    // The flips module is on by default, so Northwind's board shows a live
    // acquisition pipeline with real underwriting the moment you open it.
    let dd_checklist = serde_json::json!([
        { "key": "inspection", "label": "General inspection", "done": true },
        { "key": "title", "label": "Title search / commitment", "done": true },
        { "key": "bids", "label": "Contractor rehab bids", "done": false },
        { "key": "financing", "label": "Financing commitment", "done": false },
        { "key": "insurance", "label": "Insurance quote", "done": false }
    ]);
    seed_deal(
        db,
        northwind,
        "Elm Street Duplex",
        "412 Elm St",
        "Columbus",
        "prospecting",
        "rental",
        "multi_family",
        28_500_000,
        None,
        32_000_000,
        3_500_000,
        800_000,
        320_000,
        95_000,
        650,
        serde_json::json!([]),
        now,
    )
    .await?;
    seed_deal(
        db,
        northwind,
        "Oak & 3rd Flip",
        "1207 Oak Ave",
        "Columbus",
        "under_contract",
        "flip",
        "single_family",
        21_000_000,
        Some(19_800_000),
        31_500_000,
        6_000_000,
        600_000,
        0,
        0,
        0,
        dd_checklist,
        now,
    )
    .await?;
    seed_deal(
        db,
        northwind,
        "Birch Lane BRRRR",
        "88 Birch Ln",
        "Dublin",
        "prospecting",
        "brrrr",
        "single_family",
        16_500_000,
        None,
        24_000_000,
        4_500_000,
        500_000,
        185_000,
        52_000,
        600,
        serde_json::json!([]),
        now,
    )
    .await?;

    // ---- Property media: a hero photo + a second shot for the gallery ----
    seed_property_photo(
        db,
        northwind,
        maple_court,
        "front-elevation.svg",
        "Maple Court",
        "Front elevation",
        "#6366f1",
        "#0ea5e9",
        true,
        now,
    )
    .await?;
    seed_property_photo(
        db,
        northwind,
        maple_court,
        "courtyard.svg",
        "Maple Court",
        "Courtyard",
        "#0ea5e9",
        "#10b981",
        false,
        now,
    )
    .await?;
    seed_property_photo(
        db,
        northwind,
        riverside_flats,
        "riverside.svg",
        "Riverside Flats",
        "River frontage",
        "#f59e0b",
        "#ef4444",
        true,
        now,
    )
    .await?;

    // ---- Rehab / construction: a live project with a funded draw + waiver ----
    seed_rehab(db, northwind, maple_court, now).await?;

    // ---- SaaS platform billing: two past invoices per workspace so the
    // billing console + subscription page aren't empty on a fresh install.
    // The older one is settled; the most recent stays open (payable).
    let today = now.date_naive();
    let last_period = crate::saas::previous_month(today);
    let prior_anchor = chrono::NaiveDate::from_ymd_opt(
        last_period[..4].parse().unwrap_or_else(|_| today.year()),
        last_period[5..].parse().unwrap_or(1),
        1,
    )
    .unwrap_or(today);
    let prior_period = crate::saas::previous_month(prior_anchor);
    for tid in [northwind, cascade] {
        if let Some(tenant) = Tenant::find_by_id(tid).one(db).await? {
            let paid = crate::saas::generate_invoice(db, &tenant, &prior_period).await?;
            let mut am: entity::platform_invoice::ActiveModel = paid.into();
            am.status = Set("paid".into());
            am.paid_at = Set(Some(now.into()));
            am.updated_at = Set(now.into());
            am.update(db).await?;
            crate::saas::generate_invoice(db, &tenant, &last_period).await?;
        }
    }

    tracing::info!("seed: complete");
    Ok(())
}

/// Seed one acquisition deal (plus its `created` event) with sensible default
/// financing/projection knobs; the caller varies the headline economics.
#[allow(clippy::too_many_arguments)]
async fn seed_deal(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    address: &str,
    city: &str,
    stage: &str,
    strategy: &str,
    property_type: &str,
    asking_cents: i64,
    offer_cents: Option<i64>,
    arv_cents: i64,
    rehab_cents: i64,
    closing_cents: i64,
    rent_cents: i64,
    expenses_cents: i64,
    exit_cap_bps: i32,
    checklist: serde_json::Value,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let opt = |c: i64| if c > 0 { Some(c) } else { None };
    let opt_bps = |b: i32| if b > 0 { Some(b) } else { None };
    entity::deal::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        name: Set(name.into()),
        address: Set(address.into()),
        city: Set(city.into()),
        stage: Set(stage.into()),
        strategy: Set(strategy.into()),
        property_type: Set(Some(property_type.into())),
        source: Set(Some("mls".into())),
        broker_id: Set(None),
        notes: Set(None),
        asking_price_cents: Set(opt(asking_cents)),
        offer_price_cents: Set(offer_cents),
        earnest_money_cents: Set(None),
        target_close_on: Set(None),
        arv_cents: Set(opt(arv_cents)),
        rehab_budget_cents: Set(opt(rehab_cents)),
        closing_costs_cents: Set(opt(closing_cents)),
        est_monthly_rent_cents: Set(opt(rent_cents)),
        est_monthly_expenses_cents: Set(opt(expenses_cents)),
        vacancy_bps: Set(Some(500)),
        down_payment_bps: Set(Some(2500)),
        interest_rate_bps: Set(Some(725)),
        loan_term_years: Set(Some(30)),
        rent_growth_bps: Set(Some(300)),
        appreciation_bps: Set(Some(350)),
        exit_cap_rate_bps: Set(opt_bps(exit_cap_bps)),
        selling_costs_bps: Set(Some(700)),
        hold_years: Set(Some(5)),
        checklist: Set(checklist),
        converted_property_id: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    entity::deal_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        deal_id: Set(id),
        kind: Set("created".into()),
        from_stage: Set(None),
        to_stage: Set(Some(stage.into())),
        body: Set(None),
        actor_user_id: Set(None),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    Ok(id)
}

/// A lightweight SVG "photo" placeholder — a labelled gradient — so seeded media
/// renders as a real image (the blob route serves the document's `image/svg+xml`
/// content type). Real deployments upload real photos through the same path.
fn placeholder_svg(title: &str, subtitle: &str, c1: &str, c2: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="500" viewBox="0 0 800 500">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
<stop offset="0" stop-color="{c1}"/><stop offset="1" stop-color="{c2}"/></linearGradient></defs>
<rect width="800" height="500" fill="url(#g)"/>
<rect x="0" y="380" width="800" height="120" fill="#00000059"/>
<text x="40" y="432" font-family="system-ui,Arial" font-size="40" fill="#ffffff" font-weight="700">{title}</text>
<text x="40" y="472" font-family="system-ui,Arial" font-size="24" fill="#e5e7eb">{subtitle}</text>
</svg>"##
    )
}

/// Seed one property **photo** (stored in the object store) and optionally make
/// it the property's hero. Demonstrates the Phase 7 media surface end to end.
#[allow(clippy::too_many_arguments)]
async fn seed_property_photo(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    filename: &str,
    title: &str,
    subtitle: &str,
    c1: &str,
    c2: &str,
    make_hero: bool,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Uuid> {
    let bytes = placeholder_svg(title, subtitle, c1, c2).into_bytes();
    let doc_id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{doc_id}");
    crate::storage::ObjectStore::from_env()?
        .put_bytes(&storage_key, &bytes)
        .await?;

    entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(tenant_id),
        owner_type: Set("property".into()),
        owner_id: Set(property_id),
        filename: Set(filename.into()),
        category: Set(Some("photo".into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set("image/svg+xml".into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(crate::storage::sha256_hex(&bytes))),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(storage_key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    if make_hero {
        if let Some(p) = Property::find_by_id(property_id).one(db).await? {
            let mut am: entity::property::ActiveModel = p.into();
            am.image_url = Set(Some(format!("doc:{doc_id}")));
            am.update(db).await?;
        }
    }

    Ok(doc_id)
}

/// Seed a demo rehab project on a property: a budget with scope lines, an
/// approved change order, a funded draw, and a generated lien waiver PDF — the
/// full issue #40 loop.
async fn seed_rehab(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    property_id: Uuid,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let contractor = Uuid::new_v4();
    entity::counterparty::ActiveModel {
        id: Set(contractor),
        tenant_id: Set(tenant_id),
        kind: Set("contractor".into()),
        name: Set("Ridgeline Construction".into()),
        contact_name: Set(Some("Dana Ruiz".into())),
        email: Set(Some("dana@ridgeline.example".into())),
        phone: Set(None),
        website: Set(None),
        address: Set(None),
        notes: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    let project = Uuid::new_v4();
    entity::rehab_project::ActiveModel {
        id: Set(project),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        name: Set("Unit turns + roof".into()),
        status: Set("active".into()),
        budget_cents: Set(6_500_000),
        contingency_bps: Set(1000),
        start_date: Set(None),
        target_end_date: Set(None),
        notes: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    for (i, (cat, amt)) in [
        ("Roof replacement", 1_800_000i64),
        ("Kitchen refresh", 2_200_000),
        ("Paint & flooring", 1_500_000),
    ]
    .iter()
    .enumerate()
    {
        entity::rehab_line::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            project_id: Set(project),
            category: Set((*cat).into()),
            description: Set(None),
            budget_cents: Set(*amt),
            sort_order: Set(i as i32),
            created_at: Set(now.into()),
        }
        .insert(db)
        .await?;
    }

    entity::rehab_change_order::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        project_id: Set(project),
        description: Set("Additional electrical panel".into()),
        amount_cents: Set(300_000),
        status: Set("approved".into()),
        created_by: Set(None),
        approved_by: Set(None),
        created_at: Set(now.into()),
        decided_at: Set(Some(now.into())),
    }
    .insert(db)
    .await?;

    let draw = Uuid::new_v4();
    entity::rehab_draw::ActiveModel {
        id: Set(draw),
        tenant_id: Set(tenant_id),
        project_id: Set(project),
        number: Set(1),
        title: Set("Draw 1 — demo + roof".into()),
        amount_cents: Set(2_000_000),
        status: Set("funded".into()),
        contractor_id: Set(Some(contractor)),
        notes: Set(None),
        requested_by: Set(None),
        approved_by: Set(None),
        funded_at: Set(Some(now.into())),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    let address = Property::find_by_id(property_id)
        .one(db)
        .await?
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();
    let body = crate::routes::rehab::waiver_body(
        "conditional_progress",
        "Ridgeline Construction",
        &address,
        &crate::dto::usd(2_000_000),
        None,
        &now.date_naive().format("%Y-%m-%d").to_string(),
    );
    let doc = crate::routes::rehab::store_waiver_pdf(
        db,
        tenant_id,
        draw,
        "lien-waiver-draw-1-conditional_progress.pdf",
        &body,
    )
    .await
    .map_err(|e| anyhow::anyhow!("seed lien waiver: {e}"))?;

    entity::rehab_lien_waiver::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        draw_id: Set(draw),
        project_id: Set(project),
        waiver_type: Set("conditional_progress".into()),
        contractor_id: Set(Some(contractor)),
        contractor_name: Set("Ridgeline Construction".into()),
        amount_cents: Set(2_000_000),
        through_date: Set(None),
        status: Set("generated".into()),
        document_id: Set(Some(doc)),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    Ok(())
}

/// First + last day of `d`'s month, as `YYYY-MM-DD`.
fn month_bounds(d: chrono::NaiveDate) -> (String, String) {
    let first = chrono::NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d);
    let last = first
        .checked_add_months(chrono::Months::new(1))
        .and_then(|n| n.pred_opt())
        .unwrap_or(first);
    (first.to_string(), last.to_string())
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

#[allow(clippy::too_many_arguments)]
async fn seed_lease_payment(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lease_id: Uuid,
    kind: &str,
    due_date: &str,
    amount_cents: i64,
    status: &str,
    paid_date: Option<&str>,
    method: Option<&str>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    let receipt = paid_date.map(|d| {
        format!(
            "RCT-{}-{}",
            &d[..4],
            id.simple().to_string()[..8].to_uppercase()
        )
    });
    entity::lease_payment::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        lease_id: Set(lease_id),
        due_date: Set(due_date.into()),
        amount_cents: Set(amount_cents),
        paid_date: Set(paid_date.map(str::to_string)),
        status: Set(status.into()),
        method: Set(method.map(str::to_string)),
        created_at: Set(Utc::now().into()),
        kind: Set(kind.into()),
        method_id: Set(None),
        provider: Set(None),
        external_id: Set(None),
        failure_reason: Set(None),
        receipt_number: Set(receipt),
        ledger_txn_id: Set(None),
    }
    .insert(db)
    .await?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn seed_payment_method(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    lease_id: Uuid,
    user_id: Option<Uuid>,
    kind: &str,
    brand: Option<&str>,
    last4: &str,
    autopay: bool,
    autopay_day: Option<i32>,
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::payment_method::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        lease_id: Set(Some(lease_id)),
        user_id: Set(user_id),
        provider: Set("simulated".into()),
        kind: Set(kind.into()),
        external_id: Set(format!("sim_pm_{}{last4}", id.simple())),
        brand: Set(brand.map(str::to_string)),
        last4: Set(last4.into()),
        exp_month: Set(Some(12)),
        exp_year: Set(Some(2028)),
        status: Set("active".into()),
        autopay: Set(autopay),
        autopay_day: Set(autopay_day),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
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
        location: Set(None),
        access_notes: Set(None),
        permission_to_enter: Set(false),
        asset_id: Set(None),
        waiting_on: Set(None),
        follow_up_date: Set(None),
        rating: Set(None),
        review_comment: Set(None),
        reviewed_at: Set(None),
        due_date: Set(None),
        cost_cents: Set(None),
        first_response_at: Set(Some(now.into())),
        resolved_at: Set(if status == "resolved" || status == "closed" {
            Some(now.into())
        } else {
            None
        }),
        sla_response_due_at: Set(Some((now + chrono::Duration::hours(8)).into())),
        sla_resolve_due_at: Set(Some((now + chrono::Duration::hours(72)).into())),
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
        has_pet: Set(false),
        pet_details: Set(None),
        is_military: Set(false),
        annual_income_cents: Set(None),
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
        // Platform defaults (api::notify) apply until a tenant overrides a key.
        notification_templates: Set(json!({})),
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
        email_dns_status: Set(json!({})),
        email_verified_at: Set(None),
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
) -> anyhow::Result<Uuid> {
    let id = Uuid::new_v4();
    entity::bank_account::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        entity_id: Set(entity_id),
        kind: Set(kind.into()),
        institution: Set(institution.into()),
        masked_number: Set(Some(format!("••••{last4}"))),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
        provider: Set(None),
        external_id: Set(None),
        last_synced_at: Set(None),
    }
    .insert(db)
    .await?;
    Ok(id)
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
        image_url: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// Set a property's hero photo (partial update — only `image_url` changes).
async fn seed_property_image(
    db: &DatabaseConnection,
    property_id: Uuid,
    url: &str,
) -> anyhow::Result<()> {
    entity::property::ActiveModel {
        id: Set(property_id),
        image_url: Set(Some(url.into())),
        ..Default::default()
    }
    .update(db)
    .await?;
    Ok(())
}

/// File a document against an owner record. Demo rows carry metadata only (no
/// blob) so the documents tab lists them; wet-ink originals record where the
/// paper lives.
#[allow(clippy::too_many_arguments)]
async fn seed_document(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    owner_type: &str,
    owner_id: Uuid,
    filename: &str,
    category: &str,
    requires_wet_ink: bool,
    physical_location: Option<&str>,
) -> anyhow::Result<()> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set(owner_type.into()),
        owner_id: Set(owner_id),
        filename: Set(filename.into()),
        category: Set(Some(category.into())),
        requires_wet_ink: Set(requires_wet_ink),
        physical_location: Set(physical_location.map(|s| s.to_string())),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(0),
        checksum: Set(None),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(format!("{tenant_id}/{id}")),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(())
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
