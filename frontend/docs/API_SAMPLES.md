# Live API response samples (seeded data)

Reference for building pages. Captured from the running backend.

## GET /properties (item 0)

```json
{
  "id": "b3225281-c62e-4476-965b-213a3dc5d7f1",
  "name": "Birchwood Lofts",
  "address": "88 Birch Ave",
  "city": "Portland, OR",
  "llc_id": "f8f4b9e8-93da-4330-9070-8d595c4ddc6f",
  "units": 12,
  "occupied_units": 11,
  "occupancy": "11/12",
  "monthly_rent_cents": 1790000,
  "monthly_rent_label": "$17,900",
  "status": "Vacant",
  "year_built": 2019,
  "manager": "Dana K.",
  "property_type": "multi_family",
  "strategy": "rental",
  "workflow_stage": "managing",
  "purchase_price_cents": null,
  "acquired_on": null
}
```

## GET /properties/{id} (full profile)

```json
{
  "id": "b3225281-c62e-4476-965b-213a3dc5d7f1",
  "name": "Birchwood Lofts",
  "address": "88 Birch Ave",
  "city": "Portland, OR",
  "llc_id": "f8f4b9e8-93da-4330-9070-8d595c4ddc6f",
  "units": 12,
  "occupied_units": 11,
  "occupancy": "11/12",
  "monthly_rent_cents": 1790000,
  "monthly_rent_label": "$17,900",
  "status": "Vacant",
  "year_built": 2019,
  "manager": "Dana K.",
  "property_type": "multi_family",
  "strategy": "rental",
  "workflow_stage": "managing",
  "purchase_price_cents": null,
  "acquired_on": null,
  "kpis": [
    {
      "label": "Monthly rent",
      "amount_cents": 1790000,
      "amount_label": "$17,900"
    },
    {
      "label": "Occupancy",
      "amount_cents": 11,
      "amount_label": "11/12"
    },
    {
      "label": "Net revenue",
      "amount_cents": 1270900,
      "amount_label": "$12,709"
    },
    {
      "label": "Maintenance MTD",
      "amount_cents": 161100,
      "amount_label": "$1,611"
    }
  ],
  "cost_breakdown": [
    {
      "label": "Rent income",
      "amount_cents": 1790000,
      "amount_label": "$17,900"
    },
    {
      "label": "Maintenance & repairs",
      "amount_cents": -161100,
      "amount_label": "-$1,611"
    },
    {
      "label": "Taxes & insurance",
      "amount_cents": -214800,
      "amount_label": "-$2,148"
    },
    {
      "label": "Management fee (8%)",
      "amount_cents": -143200,
      "amount_label": "-$1,432"
    }
  ],
  "net_revenue_cents": 1270900,
  "net_revenue_label": "$12,709",
  "financed": false,
  "debt_service_cents": 0,
  "debt_service_label": "$0",
  "cash_flow_cents": 1270900,
  "cash_flow_label": "$12,709",
  "total_loan_balance_cents": 0,
  "total_loan_balance_label": "$0",
  "equity_cents": 0,
  "equity_label": "$0"
}
```

## GET /properties/{id}/intel

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /properties/{id}/units

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /properties/{id}/mortgages

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /properties/{id}/ownership

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /properties/{id}/liens

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /properties/{id}/workflow

```json
{
  "__http_error__": 404,
  "body": "{\"error\":{\"code\":\"not_found\",\"message\":\"property not found\"}}"
}
```

## GET /leases (item 0 + count)

```json
{
  "count": 2,
  "item0": {
    "id": "745362d7-d30e-41cc-88d4-bcdd33070708",
    "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
    "property_id": "06510089-661d-4df1-8d50-bdbcc7b14581",
    "unit_id": "2fe0815a-863b-4af2-8b65-a930a9203316",
    "tenant_name": "Jordan Avery",
    "tenant_email": "jordan.a@example.com",
    "tenant_phone": null,
    "rent_cents": 162000,
    "rent_label": "$1,620",
    "deposit_cents": 162000,
    "deposit_label": "$1,620",
    "start_date": "2024-06-15",
    "end_date": null,
    "status": "active",
    "payment_status": "late",
    "balance_cents": 162000,
    "notes": null,
    "created_at": "2026-06-29T01:14:00.879654+00:00",
    "updated_at": "2026-06-29T01:14:00.879654+00:00"
  }
}
```

## GET /leases/{id} (detail)

```json
{
  "id": "745362d7-d30e-41cc-88d4-bcdd33070708",
  "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
  "property_id": "06510089-661d-4df1-8d50-bdbcc7b14581",
  "unit_id": "2fe0815a-863b-4af2-8b65-a930a9203316",
  "tenant_name": "Jordan Avery",
  "tenant_email": "jordan.a@example.com",
  "tenant_phone": null,
  "rent_cents": 162000,
  "rent_label": "$1,620",
  "deposit_cents": 162000,
  "deposit_label": "$1,620",
  "start_date": "2024-06-15",
  "end_date": null,
  "status": "active",
  "payment_status": "late",
  "balance_cents": 162000,
  "notes": null,
  "created_at": "2026-06-29T01:14:00.879654+00:00",
  "updated_at": "2026-06-29T01:14:00.879654+00:00",
  "payments": [
    {
      "id": "d700a995-9cde-4893-90c8-9d09aea1dc8c",
      "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
      "lease_id": "745362d7-d30e-41cc-88d4-bcdd33070708",
      "due_date": "2025-06-01",
      "amount_cents": 162000,
      "amount_label": "$1,620",
      "paid_date": null,
      "status": "late",
      "method": null,
      "created_at": "2026-06-29T01:14:00.884696+00:00"
    }
  ]
}
```

## GET /tickets (item 0 + count)

```json
{
  "count": 1,
  "item0": {
    "id": "41755fdb-753c-42a7-ba48-380a627cf9bd",
    "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
    "property_id": "06510089-661d-4df1-8d50-bdbcc7b14581",
    "unit_id": "2fe0815a-863b-4af2-8b65-a930a9203316",
    "lease_id": null,
    "title": "Kitchen faucet leaking",
    "description": null,
    "category": "plumbing",
    "priority": "high",
    "status": "in_progress",
    "assignee_user_id": null,
    "assignee_entity_id": "2be30989-1ef1-4c06-a1b6-a2d80f02ba79",
    "reporter": "Resident",
    "due_date": null,
    "cost_cents": null,
    "cost_label": null,
    "created_at": "2026-06-29T01:14:00.890678+00:00",
    "updated_at": "2026-06-29T01:14:00.890678+00:00"
  }
}
```

## GET /tickets/{id} (detail)

```json
{
  "id": "41755fdb-753c-42a7-ba48-380a627cf9bd",
  "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
  "property_id": "06510089-661d-4df1-8d50-bdbcc7b14581",
  "unit_id": "2fe0815a-863b-4af2-8b65-a930a9203316",
  "lease_id": null,
  "title": "Kitchen faucet leaking",
  "description": null,
  "category": "plumbing",
  "priority": "high",
  "status": "in_progress",
  "assignee_user_id": null,
  "assignee_entity_id": "2be30989-1ef1-4c06-a1b6-a2d80f02ba79",
  "reporter": "Resident",
  "due_date": null,
  "cost_cents": null,
  "cost_label": null,
  "created_at": "2026-06-29T01:14:00.890678+00:00",
  "updated_at": "2026-06-29T01:14:00.890678+00:00",
  "comments": []
}
```

## GET /entities (item 0 + count)

```json
{
  "count": 3,
  "item0": {
    "id": "2be30989-1ef1-4c06-a1b6-a2d80f02ba79",
    "kind": "contractor",
    "name": "Birch & Co. General Contracting",
    "contact_name": "Sam Ortiz",
    "email": null,
    "phone": "(503) 555-0177",
    "website": null,
    "address": null,
    "notes": null,
    "created_at": "2026-06-29T01:14:00.851385+00:00",
    "updated_at": "2026-06-29T01:14:00.851385+00:00"
  }
}
```

## GET /entities/{id} (detail)

```json
{
  "id": "2be30989-1ef1-4c06-a1b6-a2d80f02ba79",
  "kind": "contractor",
  "name": "Birch & Co. General Contracting",
  "contact_name": "Sam Ortiz",
  "email": null,
  "phone": "(503) 555-0177",
  "website": null,
  "address": null,
  "notes": null,
  "created_at": "2026-06-29T01:14:00.851385+00:00",
  "updated_at": "2026-06-29T01:14:00.851385+00:00",
  "notes_log": []
}
```

## GET /portfolio/llcs

```json
[]
```

## GET /applications

```json
[]
```

## GET /members

```json
[
  {
    "membership_id": "f2241819-2598-457c-bd2b-483514fb7ba3",
    "user_id": "8e7df0fb-795f-4815-8492-365a134efc9b",
    "name": "Jordan Mills",
    "email": "jordan@northwind.com",
    "profile_type": "tenant_owner",
    "title": "Principal",
    "status": "active"
  },
  {
    "membership_id": "e7e8ece7-5e7a-4502-88de-1fc05e96990a",
    "user_id": "f118560e-089c-4202-ac31-be07ceae8c28",
    "name": "Morgan Lee",
    "email": "morgan@northwind.com",
    "profile_type": "back_office",
    "title": "Operations",
    "status": "active"
  },
  {
    "membership_id": "e1e7cdf8-5196-49c4-99b5-81bfba513d2b",
    "user_id": "f5d8bcdc-2a64-46e2-9a86-ba0012f02ab4",
    "name": "Lee Carter",
    "email": "lee@northwind.com",
    "profile_type": "landlord",
    "title": "Owner \u2014 Maple Holdings",
    "status": "active"
  }
]
```

## GET /modules

```json
[
  {
    "key": "properties",
    "name": "Property Management",
    "description": "Portfolio, onboarding, property profiles, financing, investment workflows, and LLC holding entities.",
    "permissions": [
      "property:read",
      "property:write",
      "finance:read",
      "finance:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "property_intel",
    "name": "Property Intelligence",
    "description": "Parcel/county records, taxes, valuations (AVM), schools & utilities \u2014 fetched and validated automatically.",
    "permissions": [
      "property:read",
      "property:write"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "entities",
    "name": "Entities & Contacts",
    "description": "Registry of banks, lenders, contractors and other counterparties, with notes.",
    "permissions": [
      "entity:read",
      "entity:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "llc_onboarding",
    "name": "LLC Onboarding",
    "description": "Onboard holding companies: documents, branding, signature blocks, and auto-generated leases & letters.",
    "permissions": [
      "llc:read",
      "llc:manage",
      "storage:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "rentals",
    "name": "Rentals & Leasing",
    "description": "Units, leases, and the rent payment ledger for a rental portfolio.",
    "permissions": [
      "lease:read",
      "lease:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "maintenance",
    "name": "Maintenance & Work Orders",
    "description": "Repair/turn tickets against properties, units and leases, assignable to members or contractors, with a comment timeline.",
    "permissions": [
      "maintenance:read",
      "maintenance:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "title",
    "name": "Title & Ownership",
    "description": "Ownership of record (deed/vesting, fractional shares) and liens recorded against a property's title.",
    "permissions": [
      "title:read",
      "title:manage"
    ],
    "enabled": true,
    "default_enabled": true,
    "preview": false
  },
  {
    "key": "leasing",
    "name": "Leasing & Listings",
    "description": "Public listings website, applications, and tenant screening.",
    "permissions": [
      "listing:read",
      "application:read"
```

## GET /api-tokens

```json
[]
```

## GET /storage/config

```json
{
  "provider": "platform",
  "bucket": null,
  "region": null,
  "prefix": null,
  "endpoint": null,
  "has_credentials": false,
  "is_default": true
}
```

## GET /admin/permissions (count + item0)

```json
{
  "count": 34,
  "item0": {
    "key": "audit:read",
    "category": "Access",
    "label": "View audit log",
    "description": "View the security audit trail (PII reveals, role/user changes).",
    "scope": "both"
  }
}
```

## GET /admin/roles (item0 + count)

```json
{
  "count": 12,
  "item0": {
    "id": "16ca2b21-04c0-4bfa-8d49-f3d92dadef4e",
    "scope": "platform",
    "tenant_id": null,
    "key": "acre_account_manager",
    "name": "Account Manager",
    "description": "Manage client accounts, users, and access.",
    "is_system": true,
    "permissions": [
      "tenant:manage",
      "user:read",
      "user:manage",
      "profile:read",
      "member:read",
      "member:manage",
      "role:read",
      "billing:read",
      "property:read",
      "application:read",
      "audit:read"
    ]
  }
}
```

## GET /admin/users (staff; item0 + count)

```json
{
  "count": 6,
  "item0": {
    "id": "59e553c3-016b-4a97-90f1-bdd657f7dadb",
    "email": "avery@acrehq.com",
    "username": null,
    "name": "Avery Stone",
    "status": "active",
    "is_platform_staff": true,
    "tenant_id": null
  }
}
```

## GET /admin/audit?limit=3 (staff)

```json
[
  {
    "id": "99f45839-be45-4366-9f50-ff122cb67042",
    "actor_user_id": null,
    "actor_name": null,
    "action": "http.request",
    "target_type": null,
    "target_id": null,
    "tenant_id": null,
    "metadata": null,
    "principal_kind": "public",
    "method": "POST",
    "path": "/auth/login",
    "status_code": 200,
    "ip": "127.0.0.1",
    "duration_ms": 725,
    "request_id": "7e29225e-01bd-43a9-ac7c-1ee770d8495f",
    "created_at": "2026-06-29T02:27:23.731062+00:00"
  },
  {
    "id": "43e37077-158c-43ed-b95d-9a723faebb35",
    "actor_user_id": "59e553c3-016b-4a97-90f1-bdd657f7dadb",
    "actor_name": "Avery Stone",
    "action": "auth.login",
    "target_type": "user",
    "target_id": "59e553c3-016b-4a97-90f1-bdd657f7dadb",
    "tenant_id": null,
    "metadata": null,
    "principal_kind": "user",
    "method": null,
    "path": null,
    "status_code": null,
    "ip": null,
    "duration_ms": null,
    "request_id": null,
    "created_at": "2026-06-29T02:27:23.725701+00:00"
  },
  {
    "id": "8cfef3a3-c8a8-42e1-b3f4-5f99eefbace8",
    "actor_user_id": "8e7df0fb-795f-4815-8492-365a134efc9b",
    "actor_name": "Jordan Mills",
    "action": "http.request",
    "target_type": null,
    "target_id": null,
    "tenant_id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
    "metadata": null,
    "principal_kind": "user",
    "method": "GET",
    "path": "/admin/roles",
    "status_code": 200,
    "ip": "127.0.0.1",
    "duration_ms": 42,
    "request_id": "3a4e6fec-1cd9-4456-9986-2ae196aa4cdb",
    "created_at": "2026-06-29T02:27:23.001317+00:00"
  }
]
```

## GET /platform/tenants (staff)

```json
[
  {
    "id": "a3ad2fa9-6b25-4188-9762-259379625903",
    "slug": "cascade",
    "name": "Cascade Living LLC",
    "plan": "starter",
    "status": "active",
    "custom_domain": null,
    "property_count": 0,
    "managed_revenue_label": "$0"
  },
  {
    "id": "50db1ed7-c31f-422d-8a4f-5e1d1d25a0b2",
    "slug": "northwind",
    "name": "Northwind Property Group",
    "plan": "growth",
    "status": "active",
    "custom_domain": null,
    "property_count": 0,
    "managed_revenue_label": "$0"
  }
]
```

## GET /platform/metrics (staff)

```json
{
  "tenant_count": 2,
  "active_tenants": 2,
  "total_properties": 0,
  "total_managed_revenue_label": "$0"
}
```
