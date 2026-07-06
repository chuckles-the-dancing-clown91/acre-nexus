# Tenant Screening & FCRA Adverse Action (Phase 4)

How an application becomes a screening decision: applicant consent at intake,
a real consumer report ordered through the Checkr provider (credit + criminal
+ eviction), the workspace's screening policy, and the FCRA adverse-action
workflow when a report costs someone the apartment. Roadmap Phase 4, epic #8.

Everything rides the Phase 1 substrate: the provider is **sandbox-first**
(deterministic simulation by default, live via `LIVE_PROVIDERS=checkr`),
completion arrives on the signature-verified webhook endpoint, the pipeline is
a durable retrying background job, and every step writes a domain audit event.
The screening endpoints belong to the `leasing` module.

## Consent first (FCRA §604(b))

A consumer report may only be ordered with the applicant's authorization, so
consent is captured at **every intake door** and stamped on the application
(`application.screening_consent_at`):

| Door | How consent is captured |
| --- | --- |
| Public website (`POST /public/applications`) | `screening_consent: true` in the request — the apply form's checkbox. Without it the API refuses the application (400). |
| Renter portal (`POST /my/applications`) | Same `screening_consent` flag, same refusal without it. |
| Back office (`POST /applications`) | Staff intake implies the signed paper authorization was collected outside the system (`screening_consent` defaults to true; can be explicitly negated). |
| Reuse (`POST /applications/reuse`) | No re-screen happens; the prior application's consent (and screening result) carry forward. |

## The pipeline

Submitting an application enqueues the same `background_check` job Phase 2
established — Phase 4 replaced its body (`crate::screening::handle_job`):

```
pending            order the report: create screening_report (one per
                   application), audit screening.ordered; live mode calls
                   Checkr now so the webhook has something to complete
awaiting_callback  simulation: the provider answers deterministically now.
                   live: wait for POST /webhooks/checkr (report.completed),
                   checking in every 5 minutes
   └─ land         write results onto the report, evaluate the workspace
                   policy, stamp application.screening_status/screened_at,
                   audit screening.completed + application.screened, then
                   auto-approve (applications.auto_approve) or notify staff
```

The report row (`screening_report`) stores what came back — credit score,
criminal/eviction record counts, the provider's `clear`/`consider` assessment
— plus our policy verdict (`cleared`/`failed`) and the reasons that tripped.
One report per application; retries and the webhook path converge on the same
row idempotently.

### The policy

`screening::evaluate_policy` is pure and unit-tested. A report fails when any
of these trip (checks without data never trip):

- credit score below `screening.min_credit_score` (0 disables),
- annual income below `screening.min_income_rent_ratio` × the listing's rent
  × 12 (0 disables; needs a listing with a rent),
- any criminal records reported,
- any eviction records reported.

### The provider

`providers/screening.rs` — Checkr behind the standard `Provider` trait:

- **Simulated** (default): a deterministic bureau derived from a SHA-256 of
  the applicant's email — same applicant, same report, every run. A stated
  credit score wins (it's what the tenant's policy already saw); otherwise
  580–839 from the hash. Records are rare (`h % 29`, `h % 41`); an email
  containing `flag` always trips one criminal + one eviction record — the
  demo lever, like Stripe's `0002` card.
- **Live**: `LIVE_PROVIDERS=checkr` + the `checkr.api_key` credential in the
  secrets vault. Ordering `POST /v1/reports` is idempotent on our
  `screening_report.id` (the `reference` in metadata); the completed report
  arrives on `POST /webhooks/checkr` (HMAC-verified with
  `webhook.checkr.secret`) and lands through the exact same code path as the
  simulation.

Requests carry identity attributes only — never SSNs through our storage.

## Adverse action (FCRA §615(a))

Declining an applicant based on a consumer report requires telling them which
agency furnished the report and what their rights are. The workflow:

1. **Trigger** — automatic on decline when `screening.auto_adverse_action` is
   on (default) and the report carried adverse information (failed verdict or
   any records); or staff send it from the console
   (`POST /applications/<id>/adverse-action`, `application:write`).
2. **Notice** — generated from the report's reasons + the CRA settings
   (`screening.cra_name`, `screening.cra_contact`): the decision, the agency
   (and that it didn't make the decision), the free-copy-within-60-days and
   dispute rights.
3. **Filed** — rendered to PDF and stored as a document against the
   application (`adverse-action-notice.pdf`); the application is stamped
   (`adverse_action_at`, `adverse_action_document_id`) so it can never
   double-send (409 on retry).
4. **Sent** — the `adverse_action` email template goes to the applicant
   (trigger-keyed, so it can't double-send either), an `application_event`
   records it in the workflow history, and `application.adverse_action`
   lands in the audit log.

## Surfaces

- **Back office** — the applications inbox shows a screening badge per
  application; the *Report* panel (behind the new `screening:read`
  permission — consumer reports are more sensitive than the application
  itself) shows score/records/assessment/verdict + reasons, and carries the
  *Send adverse-action notice* button for declined applicants.
- **API** — `GET /applications/<id>/screening` (`screening:read`) returns the
  stored report; `POST /applications/<id>/adverse-action`
  (`application:write`) sends + files the notice.
- **Applicant** — consent checkbox on the apply forms; decline + adverse
  action emails.

## Settings, permissions, audit

| Setting | Default | What it does |
| --- | --- | --- |
| `screening.min_credit_score` | 0 (off) | Credit floor the policy enforces. |
| `screening.min_income_rent_ratio` | 0 (off) | Income-to-rent multiple. |
| `screening.callback_delay_secs` | 6 | Simulated bureau turnaround. |
| `screening.cra_name` | Checkr, Inc. | Agency named on adverse-action notices. |
| `screening.cra_contact` | Checkr's address | Contact block under the agency name. |
| `screening.auto_adverse_action` | on | Auto-send the notice on decline. |
| `applications.auto_approve` | off | Auto-approve when screening clears. |

Permissions: `screening:read` (new) gates the report; granted to
`tenant_owner`, `property_manager`, `back_office`, and `leasing_agent` system
roles. Audit actions: `screening.ordered`, `screening.completed`,
`application.screened` (unchanged slot), `application.adverse_action`, plus
the standard `provider.call` / `webhook.received` plumbing events.
