//! **Notifications** — templated, multi-channel messaging (roadmap issue #18,
//! expanded with tenant-configurable providers, Web Push, and the in-app
//! inbox).
//!
//! Channels and their job kinds:
//!
//! | channel  | kind         | delivered by |
//! |----------|--------------|--------------|
//! | `email`  | `auto_email` | the tenant's configured provider ([`delivery::EmailDelivery`]: Resend / SendGrid / Postmark) or the simulated fallback |
//! | `sms`    | `auto_sms`   | Twilio ([`delivery::SmsDelivery`]) or the simulated fallback |
//! | `push`   | `auto_push`  | Web Push / VAPID ([`webpush::PushDelivery`]) to every browser subscription the user holds |
//! | `chat`   | `auto_chat`  | Slack / Discord incoming webhook ([`delivery::ChatDelivery`]); skipped when none is configured |
//! | `in_app` | — (written synchronously) | the `notification` table itself; `user_id` + `read_at` power the console inbox |
//!
//! Templates render through the same `{placeholder}` engine as lease documents
//! ([`crate::leasedoc::interpolate`]); platform defaults live here and tenants
//! override per key via `theme.notification_templates`. Every send is
//! persisted to `notification` and audited, and idempotency keys keep retried
//! jobs and duplicate triggers from double-sending. The original
//! `{ "template": …, "to": … }` payload contract is unchanged.

pub mod delivery;
mod es;
pub mod webpush;

use crate::leasedoc::interpolate;
use crate::modules::JobOutcome;
use crate::providers::{self, ProviderCtx};
use chrono::Utc;
use delivery::{
    ChatDelivery, EmailDelivery, MessageRequest, MessageResponse, SimulatedEmail, SimulatedSms,
    SmsDelivery,
};
use entity::prelude::{Notification, NotificationProvider, Theme};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use webpush::{PushDelivery, PushRequest};

/// Channels a tenant can configure a delivery provider for, with the provider
/// kinds each accepts. `push` is platform-managed (VAPID) and `in_app` needs
/// no provider, so neither appears here.
pub const PROVIDER_CHANNELS: &[(&str, &[&str])] = &[
    ("email", &["resend", "sendgrid", "postmark"]),
    ("sms", &["twilio"]),
    ("chat", &["slack", "discord"]),
];

/// One default template: rendered bodies per channel, overridable per tenant
/// via `theme.notification_templates`.
pub struct DefaultTemplate {
    pub key: &'static str,
    pub subject: &'static str,
    pub body: &'static str,
    pub sms: &'static str,
}

/// The platform template catalog — every default the engine ships. The
/// templates settings API merges tenant overrides over these and lets a
/// workspace import them as editable DB copies.
pub fn default_templates() -> &'static [DefaultTemplate] {
    DEFAULT_TEMPLATES
}

/// Platform default templates. A tenant override with the same key (a plain
/// body string, or `{ "subject": …, "body": …, "sms": … }`) wins field by
/// field. The `sms` variant doubles as the short body for push, chat, and
/// in-app renditions.
const DEFAULT_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        key: "application_approved",
        subject: "Your application with {company} has been approved",
        body: "Hi {recipient},\n\nGreat news — your rental application with {company} has been \
               approved. We'll be in touch shortly with next steps.\n\n— {company}",
        sms: "{company}: good news — your rental application has been approved. \
              We'll text you next steps shortly.",
    },
    DefaultTemplate {
        key: "application_invite",
        subject: "Apply with {company}",
        body: "Hi {recipient},\n\nThanks for coming by. Here's the application; it takes a \
               few minutes and your details are already filled in:\n{apply_url}{message}\n\n\
               — {company}",
        sms: "{company}: here's the application, your details are filled in: {apply_url}",
    },
    DefaultTemplate {
        key: "application_received",
        subject: "We received your application",
        body: "Hi {recipient},\n\nThanks for applying with {company}. Your application is in \
               review and we'll notify you as soon as there's a decision.\n\n— {company}",
        sms: "{company}: thanks — we received your application and will be in touch soon.",
    },
    DefaultTemplate {
        key: "application_submitted",
        subject: "New application from {applicant}",
        body: "Hi {recipient},\n\n{applicant} just submitted a rental application. Review it in \
               the applications inbox.\n\n— {company}",
        sms: "New application from {applicant} — review it in the console.",
    },
    DefaultTemplate {
        key: "application_screened",
        subject: "Screening finished for {applicant}: {result}",
        body: "Hi {recipient},\n\nBackground screening for {applicant} has finished with \
               result: {result}. Review the application and make a decision in the \
               applications inbox.\n\n— {company}",
        sms: "Screening finished for {applicant}: {result} — review in the console.",
    },
    DefaultTemplate {
        key: "application_declined",
        subject: "Update on your application with {company}",
        body: "Hi {recipient},\n\nThank you for applying with {company}. After careful \
               review we're unable to move forward with your application at this time.\n\n\
               If you have questions, just reply to this email.\n\n— {company}",
        sms: "{company}: unfortunately we can't move forward with your application at \
              this time.",
    },
    DefaultTemplate {
        key: "adverse_action",
        subject: "Adverse action notice regarding your application",
        body: "Hi {recipient},\n\nThis notice is provided under the Fair Credit Reporting \
               Act (FCRA). Your rental application with {company} was declined based in \
               whole or in part on information in a consumer report furnished by:\n\n\
               {cra_name}\n{cra_contact}\n\nThe agency did not make this decision and \
               cannot explain why it was made. You may obtain a free copy of your report \
               from the agency within 60 days of this notice, and you may dispute any \
               inaccurate or incomplete information with them directly. The full notice \
               is on file with your application.\n\n— {company}",
        sms: "{company}: an adverse-action notice about your application was issued — \
              see your email for your FCRA rights.",
    },
    DefaultTemplate {
        key: "ticket_created",
        subject: "New maintenance ticket: {title}",
        body: "Hi {recipient},\n\nA new {priority}-priority maintenance ticket was opened: \
               {title}. Review it on the maintenance board.\n\n— {company}",
        sms: "New {priority} maintenance ticket: {title}",
    },
    // ---- Appointments ----
    DefaultTemplate {
        key: "appointment_offered",
        subject: "Pick a time: {title}",
        body: "Hi {name},\n\nWe'd like to come by for: {title} at {property}.\n\n\
               Times we can do: {windows}.\n\nPick one here: {link}\n\nIf none of \
               those work, the same link lets you suggest another time.\n\n— {company}",
        sms: "{company}: pick a time for {title}. {windows}. {link}",
    },
    DefaultTemplate {
        key: "appointment_confirmed",
        subject: "Confirmed: {title}, {when}",
        body: "Hi {name},\n\nYou're set for {when}: {title} at {property}. We'll \
               remind you before.\n\n— {company}",
        sms: "{company}: confirmed {when} for {title}.",
    },
    DefaultTemplate {
        key: "appointment_confirmed_staff",
        subject: "Scheduled: {title}, {when}",
        body: "Hi {recipient},\n\n{name} picked {when} for {title} at {property}.\n\n— {company}",
        sms: "{name} picked {when} for {title}.",
    },
    DefaultTemplate {
        key: "owner_approval_request",
        subject: "Approve {amount} of work at {property}?",
        body: "Hi {recipient},\n\n{title} at {property} is estimated at {amount}, over your \
               {limit} limit.{note}\n\nApprove or decline here:\n{link}\n\nNothing is \
               sent to a vendor until you say so.\n\n— {company}",
        sms: "{company}: approve {amount} of work at {property} ({title})? {link}",
    },
    DefaultTemplate {
        key: "owner_approval_reminder",
        subject: "Still waiting: {amount} of work at {property}",
        body: "Hi {recipient},\n\nA reminder that {title} at {property} ({amount}) is \
               waiting for your approval. Approve or decline here:\n{link}\n\n— {company}",
        sms: "{company}: still waiting on your approval for {title} at {property} ({amount}). {link}",
    },
    DefaultTemplate {
        key: "owner_signoff_request",
        subject: "Work finished at {property}: {title}",
        body: "Hi {recipient},\n\n{title} at {property} is done. The cost came to {amount}.\
               {note}\n\nSee the photos and sign off, or tell us what's not right:\n{link}\n\n\
               — {company}",
        sms: "{company}: {title} at {property} is done ({amount}). Sign off or dispute: {link}",
    },
    DefaultTemplate {
        key: "owner_approval_decided",
        subject: "{owner} {decision}: {title}",
        body: "Hi {recipient},\n\n{owner} {decision} {title} at {property} ({amount}).\
               {note}\n\n— {company}",
        sms: "{owner} {decision} {title} at {property} ({amount}).",
    },
    DefaultTemplate {
        key: "owner_statement",
        subject: "Your {month} statement from {company}",
        body: "Hi {recipient},\n\nYour statement for {month} is ready: rent collected \
               {rent}, expenses {expenses}, net {net}.\n\nRead it, with the work done on \
               your properties, here:\n{link}\n\n— {company}",
        sms: "{company}: your {month} statement is ready. Net {net}. {link}",
    },
    DefaultTemplate {
        key: "vendor_task_declined",
        subject: "{vendor} declined: {title}",
        body: "Hi {recipient},\n\n{vendor} declined the work sent to them on {title} at \
               {property}.{reason}\n\nThe tasks are open again; send them to someone else \
               from the work order.\n\n— {company}",
        sms: "{vendor} declined {title} at {property}.",
    },
    DefaultTemplate {
        key: "vendor_task_accepted",
        subject: "{vendor} accepted: {title}",
        body: "Hi {recipient},\n\n{vendor} accepted the work on {title} at {property}.{when}\
               {note}\n\n— {company}",
        sms: "{vendor} accepted {title} at {property}.{when}",
    },
    DefaultTemplate {
        key: "vendor_task_done",
        subject: "{vendor} finished: {title}",
        body: "Hi {recipient},\n\n{vendor} marked their work on {title} at {property} \
               done.{note}{invoice}\n\nReview it from the work order.\n\n— {company}",
        sms: "{vendor} finished {title} at {property}.{invoice}",
    },
    DefaultTemplate {
        key: "ticket_rating_request",
        subject: "How did we do on \"{title}\"?",
        body: "Hi {name},\n\nWe marked \"{title}\" finished. Did it go well? A quick rating \
               helps us do better, and if anything isn't right, tell us there and we'll \
               come back.\n\n{link}\n\n— {company}",
        sms: "{company}: how did we do on \"{title}\"? Rate it or tell us what's wrong: {link}",
    },
    DefaultTemplate {
        key: "ticket_checkin",
        subject: "Still fixed? \"{title}\"",
        body: "Hi {name},\n\nIt's been a week since we finished \"{title}\". Is everything \
               still working? If not, reply on the request and we'll reopen it.\n\n{link}\n\n— {company}",
        sms: "{company}: a week on, is \"{title}\" still fixed? If not: {link}",
    },
    DefaultTemplate {
        key: "vendor_task_nudge",
        subject: "Still need an answer: {title}",
        body: "Hi {vendor},\n\nWe sent you {count} task(s) on \"{title}\" {days} day(s) ago and \
               haven't heard back:\n{tasks}\n\nCan you take it? Accept, decline or say when \
               from this link (no account needed):\n{vendor_link}\n\n— {company}",
        sms: "{company}: still need your answer on \"{title}\" ({count} tasks): {vendor_link}",
    },
    DefaultTemplate {
        key: "appointment_offer_reminder",
        subject: "Pick a time for {title}",
        body: "Hi {name},\n\nWe offered times for {title} at {property} and haven't heard \
               which works:\n{windows}\n\nPick one here, or tell us a better time:\n{link}\n\n— {company}",
        sms: "{company}: pick a time for {title}: {link}",
    },
    DefaultTemplate {
        key: "lead_after_showing",
        subject: "Ready to apply?",
        body: "Hi {name},\n\nThanks for coming to see the place. If it felt right, the \
               application takes a few minutes and your details are already filled in:\n\n{link}\n\n\
               Questions? Just reply.\n\n— {company}",
        sms: "{company}: thanks for the tour. Ready to apply? {link}",
    },
    DefaultTemplate {
        key: "route_parts_needed",
        subject: "{assignee}'s route for {when}: what to order",
        body: "Hi {recipient},\n\n{assignee}'s day for {when} is set: {stops} stops.\n\n               {to_order}{from_stock}{low}\n\nOrder from the close-out, or open the plan: {link}\n\n— {company}",
        sms: "{assignee}'s route for {when} is set; see what to order: {link}",
    },
    DefaultTemplate {
        key: "route_assigned",
        subject: "Your day for {when} is planned",
        body: "Hi {recipient},\n\n{by} planned your {when}: {stops} stops, in order, on My day.\n\n{link}\n\n— {company}",
        sms: "Your {when} is planned: {stops} stops. {link}",
    },
    DefaultTemplate {
        key: "alpha_invite",
        subject: "{company} invites you to Alpha",
        body: "Hi {recipient},\n\n{company} sends work orders through Alpha. With a free \
               Alpha account, jobs from {company} land on your own board, you can text \
               the office, and you get paid faster.\n\nSign up here:\n{join_url}\n\n\
               Until then, each work order we send comes with a link you can answer \
               from.\n\n— {company}",
        sms: "{company} invites you to Alpha. Sign up: {join_url}",
    },
    DefaultTemplate {
        key: "appointment_declined",
        subject: "Needs a new time: {title}",
        body: "Hi {recipient},\n\n{name} can't make any of the times offered for {title} \
               at {property}.{asked} {reason}\n\nOffer new times from the work order.\n\n— {company}",
        sms: "{name} can't make the offered times for {title}.{asked}",
    },
    DefaultTemplate {
        key: "appointment_reminder",
        subject: "Reminder: {title}, {when}",
        body: "Hi {name},\n\nA reminder that we're coming by {in}: {when}, for {title} \
               at {property}.\n\n— {company}",
        sms: "{company}: reminder, {title} {in}: {when}.",
    },
    DefaultTemplate {
        key: "appointment_reminder_staff",
        subject: "Up {in}: {title}, {when}",
        body: "Hi {recipient},\n\n{title} at {property} with {name} is {in}: {when}.\n\n— {company}",
        sms: "{title} at {property} is {in}: {when}.",
    },
    DefaultTemplate {
        key: "tour_requested",
        subject: "Tour request: {name} for {home}",
        body: "Hi {recipient},\n\n{name} asked to tour {home}. Preferred times: {times}. \
               Reach them at {contact}. Open Tours in the console to follow up.\n\n— {company}",
        sms: "Tour request from {name} for {home}. Contact {contact}.",
    },
    // ---- The back office ----
    DefaultTemplate {
        key: "missed_punch",
        subject: "You didn't clock out on {when}",
        body: "Hi {recipient},\n\nYour time from {when} was still running, so it was closed \
               at our best guess and held for the office. Open your hours and tell us when \
               you really finished.\n\n— {company}",
        sms: "{company}: you didn't clock out on {when} — tell us when you finished in your hours.",
    },
    DefaultTemplate {
        key: "time_unapproved",
        subject: "{count} time entries are waiting for approval",
        body: "Hi {recipient},\n\n{count} time entries from before this week still need \
               approving ({missed} missed punches among them): {people}. Approve them on the \
               timesheets page so payroll and owner billing can use them.\n\n— {company}",
        sms: "{count} time entries from before this week need approving ({missed} missed punches).",
    },
    DefaultTemplate {
        key: "time_off_submitted",
        subject: "Time off request from {employee}",
        body: "Hi {recipient},\n\n{employee} asked for {kind} from {starts_on} to {ends_on}. \
               Review it on the team schedule.\n\n— {company}",
        sms: "{employee} asked for {kind} {starts_on}–{ends_on}.",
    },
    DefaultTemplate {
        key: "time_off_reviewed",
        subject: "Your time off was {decision}",
        body: "Hi {recipient},\n\nYour {kind} from {starts_on} to {ends_on} was {decision}.\
               {note}\n\n— {company}",
        sms: "{company}: your {kind} {starts_on}–{ends_on} was {decision}.",
    },
    // ---- Two-way texts ----
    DefaultTemplate {
        key: "direct_text",
        subject: "Message from {company}",
        body: "{text}",
        sms: "{text}",
    },
    DefaultTemplate {
        key: "text_received",
        subject: "New text from {sender}",
        body: "Hi {recipient},\n\n{sender} texted: \"{preview}\"\n\nReply from the Texts inbox \
               in the console.\n\n— {company}",
        sms: "Text from {sender}: {preview}",
    },
    DefaultTemplate {
        key: "stay_requested",
        subject: "We got your request: {site}, {check_in} to {check_out}",
        body: "Hi {name},\n\nThanks for asking to stay at {campground}. We're holding {site} for \
               {check_in} to {check_out} and will confirm shortly.\n\nTotal {total}, with a \
               {deposit} deposit. See or cancel your request here:\n{link}\n\n— {company}",
        sms: "{company}: we're holding {site} at {campground} for {check_in} to {check_out}. {link}",
    },
    DefaultTemplate {
        key: "stay_confirmed",
        subject: "Confirmed: {site} at {campground}, {check_in} to {check_out}",
        body: "Hi {name},\n\nYou're booked at {campground}: {site}, {check_in} to {check_out}. \
               Total {total}, deposit {deposit}.\n\n{link}\n\nSee you soon.\n\n— {company}",
        sms: "{company}: you're booked, {site} at {campground}, {check_in} to {check_out}.",
    },
    DefaultTemplate {
        key: "stay_cancelled",
        subject: "Cancelled: {site} at {campground}, {check_in}",
        body: "Hi {name},\n\nYour stay at {campground} ({site}, {check_in} to {check_out}) is \
               cancelled. Questions about a deposit? Just reply.\n\n— {company}",
        sms: "{company}: your stay at {campground} on {check_in} is cancelled.",
    },
    DefaultTemplate {
        key: "stay_request_staff",
        subject: "Stay request: {guest}, {site}, {check_in}",
        body: "Hi {recipient},\n\n{guest} asked for {site} at {campground}, {check_in} to \
               {check_out}. The site is held; confirm or cancel it from the campground's front \
               desk.\n\n— {company}",
        sms: "Stay request: {guest}, {site} at {campground}, {check_in}.",
    },
    DefaultTemplate {
        key: "stay_cancelled_staff",
        subject: "Guest cancelled: {guest}, {site}, {check_in}",
        body: "Hi {recipient},\n\n{guest} cancelled their stay at {campground} ({site}, from \
               {check_in}). The site is free again.\n\n— {company}",
        sms: "{guest} cancelled {site} at {campground} from {check_in}.",
    },
    DefaultTemplate {
        key: "text_missed_call",
        subject: "Missed call from {sender}",
        body: "Hi {recipient},\n\nNobody picked up when {sender} called. The conversation \
               is in the Texts inbox; call or text them back from there.\n\n— {company}",
        sms: "Missed call from {sender}.",
    },
    // ---- Password links (set your password / forgot password) ----
    DefaultTemplate {
        key: "account_invite",
        subject: "{company} set up an account for you",
        body: "Hi {name},\n\n{company} has set up an account for you. Choose your password \
               here to sign in:\n\n{link}\n\nThe link works once and expires in 7 days.\n\n\
               — {company}",
        sms: "{company} set up an account for you. Choose your password: {link}",
    },
    DefaultTemplate {
        key: "password_reset",
        subject: "Reset your {company} password",
        body: "Hi {name},\n\nSomeone asked to reset the password for this account. If it was \
               you, choose a new one here:\n\n{link}\n\nThe link works once and expires in \
               24 hours. If it wasn't you, ignore this email — your password hasn't changed.\
               \n\n— {company}",
        sms: "{company}: reset your password here (expires in 24 hours): {link}",
    },
    DefaultTemplate {
        key: "test_notification",
        subject: "Test notification from {company}",
        body: "Hi {recipient},\n\nThis is a test notification from {company}. If you're reading \
               this, delivery is working.\n\n— {company}",
        sms: "{company}: test notification — delivery is working.",
    },
    // ---- E-signature envelopes (Phase 2) ----
    DefaultTemplate {
        key: "esign_request",
        subject: "Signature requested: {document_title}",
        body: "Hi {signer},\n\n{company} has requested your signature on \
               \"{document_title}\".\n\nReview and sign here:\n{sign_url}\n\nThis link is \
               unique to you — please do not forward it. By signing you agree to transact \
               electronically (ESIGN/UETA).\n\n— {company}",
        sms: "{company}: your signature is requested on {document_title}. Sign: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_reminder",
        subject: "Reminder — {document_title} is awaiting your signature",
        body: "Hi {signer},\n\nA friendly reminder that \"{document_title}\" from {company} \
               is still awaiting your signature.\n\nReview and sign here:\n{sign_url}\n\n\
               — {company}",
        sms: "{company}: reminder — {document_title} is awaiting your signature. Sign: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_signed_staff",
        subject: "{signer} signed {document_title}",
        body: "Hi {recipient},\n\n{signer} has signed \"{document_title}\" \
               ({signed_count}/{signer_count} signatures in). You'll be notified when \
               everyone has signed.\n\n— {company}",
        sms: "{signer} signed {document_title} ({signed_count}/{signer_count} signatures in).",
    },
    DefaultTemplate {
        key: "esign_completed",
        subject: "Fully signed: {document_title}",
        body: "Hi {signer},\n\nAll parties have now signed \"{document_title}\". The fully \
               executed copy is kept with the lease records at {company} — you can request \
               a copy at any time.\n\n— {company}",
        sms: "{company}: {document_title} is fully signed. The executed copy is on file.",
    },
    DefaultTemplate {
        key: "esign_completed_staff",
        subject: "{document_title} fully signed",
        body: "Hi {recipient},\n\n\"{document_title}\" is fully executed — signed by \
               {signed_by}. The signed PDF is stored on the lease and the lease is now \
               active.\n\n— {company}",
        sms: "{document_title} fully executed — signed by {signed_by}. Lease activated.",
    },
    DefaultTemplate {
        key: "esign_declined_staff",
        subject: "{signer} declined to sign {document_title}",
        body: "Hi {recipient},\n\n{signer} declined to sign \"{document_title}\"{reason_line}. \
               The envelope is closed; you can revise the document and send a new one.\n\n\
               — {company}",
        sms: "{signer} declined to sign {document_title}.",
    },
    DefaultTemplate {
        key: "esign_voided",
        subject: "Signature request cancelled: {document_title}",
        body: "Hi {signer},\n\nThe signature request for \"{document_title}\" from {company} \
               has been cancelled — no further action is needed. Your signing link no longer \
               works.\n\n— {company}",
        sms: "{company}: the signature request for {document_title} was cancelled.",
    },
    DefaultTemplate {
        key: "payment_receipt",
        subject: "Payment received — {amount}",
        body: "Hi {recipient},\n\nWe received your payment of {amount}. Your receipt number \
               is {receipt_number}; a PDF copy is kept with your lease records.\n\nThank you!\n\n\
               — {company}",
        sms: "{company}: payment of {amount} received. Receipt {receipt_number}.",
    },
    DefaultTemplate {
        key: "payment_failed",
        subject: "Your payment could not be processed",
        body: "Hi {recipient},\n\nYour payment of {amount} could not be processed: {reason}. \
               No money was taken. Please try again with another payment method, or contact \
               us if the problem persists.\n\n— {company}",
        sms: "{company}: your payment of {amount} failed ({reason}). Please try another method.",
    },
    DefaultTemplate {
        key: "autopay_failed",
        subject: "Your automatic rent payment didn't go through",
        body: "Hi {recipient},\n\nYour automatic payment of {amount} for rent due {due_date} \
               could not be processed: {reason}. No money was taken, and we won't retry this \
               payment on its own.\n\nPay now: {pay_url}\n\n— {company}",
        sms: "{company}: your autopay of {amount} failed ({reason}). Pay now: {pay_url}",
    },
    DefaultTemplate {
        key: "rent_due",
        subject: "Rent of {amount} is due {due_date}",
        body: "Hi {recipient},\n\nA reminder that your rent of {amount} is due on {due_date}.\n\n\
               Pay online: {pay_url}\n\nIf you've already paid, thank you, and please ignore \
               this.\n\n— {company}",
        sms: "{company}: rent of {amount} is due {due_date}. Pay: {pay_url}",
    },
    DefaultTemplate {
        key: "rent_past_due",
        subject: "Your rent due {due_date} is unpaid",
        body: "Hi {recipient},\n\nWe haven't received your rent of {amount} that was due on \
               {due_date}. Please pay as soon as you can to avoid a late fee.\n\nPay online: \
               {pay_url}\n\nIf you've already paid, thank you, and please ignore this.\n\n— {company}",
        sms: "{company}: rent of {amount} due {due_date} is unpaid. Pay: {pay_url}",
    },
    DefaultTemplate {
        key: "inspection_reminder",
        subject: "Your {kind} inspection is {when}",
        body: "Hi {recipient},\n\nA reminder that the {kind} inspection at {place} is \
               scheduled for {date}.\n\nAdd it to your calendar: {calendar_url}\n\nTo change \
               the time, reply to this email or message us in your portal.\n\n— {company}",
        sms: "{company}: your {kind} inspection at {place} is {date}.",
    },
    DefaultTemplate {
        key: "lease_expiring",
        subject: "Lease ending in {days} days: {tenant}, {place}",
        body: "Hi {recipient},\n\nThe lease for {tenant} at {place} ends on {end_date} \
               ({days} days). {renewal_note}\n\n— {company}",
        sms: "Lease for {tenant} at {place} ends {end_date} ({days} days).",
    },
    DefaultTemplate {
        key: "warranty_expiring",
        subject: "Warranty ending {date}: {asset}",
        body: "Hi {recipient},\n\nThe warranty on {asset} at {place} ends on {date}. If it \
               needs a claim or a service visit, now is the time.\n\n— {company}",
        sms: "Warranty on {asset} at {place} ends {date}.",
    },
    DefaultTemplate {
        key: "text_assigned",
        subject: "Text conversation with {sender} is yours",
        body: "Hi {recipient},\n\nThe text conversation with {sender} was assigned to \
               you.\n\n— {company}",
        sms: "Texts with {sender} are now yours.",
    },
    DefaultTemplate {
        key: "ticket_rate_request",
        subject: "How did we do on \"{title}\"?",
        body: "Hi {recipient},\n\nYour request \"{title}\" is done. How did we do? \
               Rate it here: {url}\n\n— {company}",
        sms: "{company}: \"{title}\" is done. How did we do? Reply 1-5 (5 is best).",
    },
    DefaultTemplate {
        key: "ticket_rating_thanks",
        subject: "Thanks for your rating",
        body: "Thanks for rating \"{title}\" {rating} out of 5.\n\n— {company}",
        sms: "Thanks! We recorded {rating}/5 for \"{title}\".",
    },
    DefaultTemplate {
        key: "repair_link",
        subject: "Send us a repair request",
        body: "Sounds like something needs fixing. Send the request here and we'll \
               get on it: {url}\n\n— {company}",
        sms: "{company}: sounds like a repair. Send it to maintenance here (already filled \
              in): {url}",
    },
    DefaultTemplate {
        key: "coi_expiring",
        subject: "Your insurance certificate ends {date}",
        body: "Hi {recipient},\n\nOur records show your {kind} policy with {carrier} ends on \
               {date}. Please send us the renewed certificate so we can keep sending you \
               work.\n\nThank you,\n{company}",
        sms: "{company}: your {kind} insurance with {carrier} ends {date}. Please send the \
              renewed certificate.",
    },
    DefaultTemplate {
        key: "vendor_coi_expiring",
        subject: "{vendor}'s insurance ends {date}",
        body: "Hi {recipient},\n\n{vendor}'s {kind} policy with {carrier} ends on {date}. \
               We've asked them for the renewed certificate.\n\n— {company}",
        sms: "{vendor}'s {kind} insurance ends {date}.",
    },
    DefaultTemplate {
        key: "manager_digest",
        subject: "Morning summary: {headline}",
        body: "Good morning {recipient},\n\n{summary}\n\nOpen the console: {console_url}\n\n— {company}",
        sms: "Morning summary: {headline}",
    },
    DefaultTemplate {
        key: "payment_received",
        subject: "Payment received: {amount} from {resident}",
        body: "Hi {recipient},\n\n{resident} paid {amount}. The payment has settled, posted \
               to the ledger, and a receipt was issued.\n\n— {company}",
        sms: "Payment settled: {amount} from {resident}.",
    },
    DefaultTemplate {
        key: "late_fee_applied",
        subject: "A late fee was applied to your account",
        body: "Hi {recipient},\n\nRent for {month} is past its grace period, and a late fee \
               of {amount} has been applied to your account per your lease terms. Paying \
               your outstanding balance stops further fees.\n\n— {company}",
        sms: "{company}: a {amount} late fee was applied for {month}. Please pay your balance.",
    },
    DefaultTemplate {
        key: "payout_paid",
        subject: "Owner payout sent: {amount}",
        body: "Hi {recipient},\n\nAn owner payout of {amount} was executed and the statement \
               is filed on the entity. The ledger entry is linked from the payout record.\n\n\
               — {company}",
        sms: "Owner payout of {amount} sent.",
    },
    DefaultTemplate {
        key: "vendor_bill_submitted",
        subject: "Vendor bill {bill_number} awaits approval: {amount}",
        body: "Hi {recipient},\n\nVendor bill {bill_number} from {vendor} for {amount} was \
               submitted and needs an approval decision. Review it in the payables \
               console.\n\n— {company}",
        sms: "Vendor bill {bill_number} from {vendor} ({amount}) awaits approval.",
    },
    DefaultTemplate {
        key: "vendor_bill_paid",
        subject: "Vendor bill {bill_number} paid: {amount}",
        body: "Hi {recipient},\n\nVendor bill {bill_number} from {vendor} was paid ({amount}). \
               The expense is posted to the entity's ledger and the linked work order was \
               updated.\n\n— {company}",
        sms: "Vendor bill {bill_number} ({amount}) to {vendor} paid.",
    },
    DefaultTemplate {
        key: "vendor_bill_remittance",
        subject: "Payment sent: {bill_number} — {amount}",
        body: "Hi {recipient},\n\nWe've sent payment of {amount} for bill {bill_number} \
               ({memo}). Depending on your bank, the transfer may take a few business days \
               to arrive.\n\n— {company}",
        sms: "{company} sent {amount} for bill {bill_number}.",
    },
    DefaultTemplate {
        key: "lead_received",
        subject: "New leasing lead: {lead_name}",
        body: "Hi {recipient},\n\n{lead_name} ({lead_email}) wrote to the leasing inbox: \
               {subject}. Follow up from the leads console.\n\n— {company}",
        sms: "New leasing lead: {lead_name} ({lead_email}) — {subject}.",
    },
    DefaultTemplate {
        key: "reminder_due",
        subject: "Reminder: {title} — due {due_date}",
        body: "Hi {recipient},\n\n{title} is due on {due_date} ({days_left} day(s) from now).\
               \n\nDetails: {description}\n\n— {company}",
        sms: "{company} reminder: {title} due {due_date} ({days_left} day(s)).",
    },
    // ---- Phase 5: resident portal round-out ----
    DefaultTemplate {
        key: "resident_message",
        subject: "New message from {resident}: {subject}",
        body: "Hi {recipient},\n\n{resident} sent a message about {property}:\n\n\
               \"{preview}\"\n\nReply from the messages console.\n\n— {company}",
        sms: "New resident message from {resident}: {subject}",
    },
    DefaultTemplate {
        key: "manager_message",
        subject: "New message from {company}: {subject}",
        body: "Hi {recipient},\n\nYou have a new message from {company} about your \
               tenancy:\n\n\"{preview}\"\n\nRead and reply from your resident portal \
               under My messages.\n\n— {company}",
        sms: "{company}: you have a new message — {subject}. Reply from your portal.",
    },
    DefaultTemplate {
        key: "maintenance_request",
        subject: "New maintenance request from {resident}: {title}",
        body: "Hi {recipient},\n\n{resident} submitted a {priority}-priority maintenance \
               request for {property}: {title}. Triage it on the maintenance board.\n\n\
               — {company}",
        sms: "New maintenance request from {resident}: {title} ({priority}).",
    },
    DefaultTemplate {
        key: "maintenance_update",
        subject: "Update on your maintenance request: {title}",
        body: "Hi {recipient},\n\nYour maintenance request \"{title}\" is now \
               {status}.\n\nYou can follow progress from your resident portal under \
               Maintenance.\n\n— {company}",
        sms: "{company}: your maintenance request \"{title}\" is now {status}.",
    },
    // ---- Phase 6: helpdesk & maintenance operations ----
    DefaultTemplate {
        key: "ticket_assigned",
        subject: "Assigned to you: {title}",
        body: "Hi {recipient},\n\nThe work order \"{title}\" ({priority} priority) at \
               {property} has been assigned to you{due_line}.\n\n— {company}",
        sms: "{company}: work order assigned to you — {title} ({priority}).",
    },
    DefaultTemplate {
        key: "ticket_dispatch",
        subject: "Work order from {company}: {title}",
        body: "Hi {recipient},\n\n{company} has dispatched a work order to you:\n\n\
               {title} ({priority} priority)\nProperty: {property}{due_line}\n\n\
               {description}\n\nAccept or decline, tell us when you can come, and send \
               photos and your invoice here (no account needed):\n{vendor_link}\n\n\
               — {company}",
        sms: "{company} dispatched a work order: {title} at {property}. Answer here: {vendor_link}",
    },
    DefaultTemplate {
        key: "maintenance_reply",
        subject: "Reply on your maintenance request: {title}",
        body: "Hi {recipient},\n\n{author} replied to your maintenance request \
               \"{title}\":\n\n\"{preview}\"\n\nRead and respond from your resident \
               portal under Maintenance.\n\n— {company}",
        sms: "{company}: {author} replied to your request \"{title}\" — see your portal.",
    },
    DefaultTemplate {
        key: "ticket_follow_up",
        subject: "Follow up due: {title} (waiting on {waiting_on})",
        body: "Hi {recipient},\n\nThe work order \"{title}\" has been waiting on \
               {waiting_on} and its follow-up date ({date}) has arrived. Chase it \
               from the maintenance board.\n\n— {company}",
        sms: "Follow up due: {title} — waiting on {waiting_on} since {date}.",
    },
    DefaultTemplate {
        key: "inventory_low",
        subject: "Low stock: {name} ({quantity} left)",
        body: "Hi {recipient},\n\nInventory for \"{name}\" is down to {quantity} \
               (reorder level {reorder_level}). Time to reorder.\n\n— {company}",
        sms: "Low stock: {name} — {quantity} left (reorder at {reorder_level}).",
    },
    DefaultTemplate {
        key: "ticket_reviewed",
        subject: "{stars} — {resident} rated \"{title}\"",
        body: "Hi {recipient},\n\n{resident} rated the completed work order \
               \"{title}\": {stars} ({rating}/5).\n\n\"{comment}\"\n\n— {company}",
        sms: "{resident} rated \"{title}\": {rating}/5.",
    },
    DefaultTemplate {
        key: "ticket_sla_breached",
        subject: "SLA breached ({kind}): {title}",
        body: "Hi {recipient},\n\nThe {priority}-priority work order \"{title}\" has \
               passed its {kind} SLA target. Triage it on the maintenance board.\n\n\
               — {company}",
        sms: "SLA breached ({kind}): {title} ({priority}).",
    },
    DefaultTemplate {
        key: "deposit_disposition_closed",
        subject: "Your security deposit statement — {refund} refunded",
        body: "Hi {recipient},\n\nYour security deposit of {deposit} has been settled: \
               {withheld} was withheld ({deduction_count} deduction(s)) and {refund} is \
               being returned to you. The itemized disposition statement is available \
               from your resident portal under My lease.\n\n— {company}",
        sms: "{company}: your deposit is settled — {refund} refunded. Statement in your portal.",
    },
];

/// A rendered, ready-to-send message. `subject` doubles as the title for
/// push/in-app renditions.
struct Rendered {
    subject: Option<String>,
    body: String,
}

/// Resolve + render `template_key` for `channel`, layering the tenant override
/// (if any) over the platform default. `None` when the key is unknown to both.
fn render(
    overrides: &serde_json::Value,
    channel: &str,
    template_key: &str,
    vars: &HashMap<&str, String>,
    lang: &str,
) -> Option<Rendered> {
    let en_default = DEFAULT_TEMPLATES.iter().find(|t| t.key == template_key);
    let en_over = overrides.get(template_key);
    // Spanish: the workspace's `<key>.es` override, then the built-in Spanish
    // version, then English for anything neither has.
    let (over, default) = if lang == "es" {
        let es_over = overrides.get(format!("{template_key}.es"));
        let es_default = es::ES_TEMPLATES.iter().find(|t| t.key == template_key);
        if es_over.is_some() || es_default.is_some() {
            (es_over, es_default.or(en_default))
        } else {
            (en_over, en_default)
        }
    } else {
        (en_over, en_default)
    };

    let str_field = |name: &str| -> Option<String> {
        over.and_then(|o| o.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    // A bare-string override is a body for every channel at once.
    let over_plain = over.and_then(|o| o.as_str()).map(str::to_string);

    // sms/chat/push/in_app use the short (`sms`) variant; email the long body.
    let body_template = match channel {
        "email" => str_field("body")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.body.to_string()))?,
        _ => str_field("sms")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.sms.to_string()))?,
    };
    // sms and chat are bare text; email, push, and in_app carry a subject/title.
    let subject = match channel {
        "sms" | "chat" => None,
        _ => Some(
            str_field("subject")
                .or(default.map(|d| d.subject.to_string()))
                .unwrap_or_else(|| "Notification from {company}".to_string()),
        ),
    };

    Some(Rendered {
        subject: subject.map(|s| interpolate(&s, vars)),
        body: interpolate(&body_template, vars),
    })
}

/// Tenant branding + template overrides for rendering.
async fn tenant_context(db: &impl ConnectionTrait, tenant_id: Uuid) -> (String, serde_json::Value) {
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    let company = theme
        .as_ref()
        .map(|t| t.company_name.clone())
        .unwrap_or_else(|| "Vantedge".to_string());
    let overrides = theme
        .map(|t| t.notification_templates)
        .unwrap_or_else(|| json!({}));
    (company, overrides)
}

/// Interpolation vars: built-ins + any string vars the caller provided.
fn build_vars(
    recipient: &str,
    company: &str,
    template: &str,
    extra: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = vec![
        ("recipient".into(), recipient.to_string()),
        ("company".into(), company.to_string()),
        ("template".into(), template.to_string()),
    ];
    if let Some(extra) = extra {
        for (k, v) in extra {
            let val = v
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string());
            pairs.push((k.clone(), val));
        }
    }
    pairs
}

/// The tenant's delivery provider for a channel: the default first, else the
/// oldest enabled one, else `None` (→ simulated fallback).
pub async fn default_provider(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    channel: &str,
) -> Option<entity::notification_provider::Model> {
    NotificationProvider::find()
        .filter(entity::notification_provider::Column::TenantId.eq(tenant_id))
        .filter(entity::notification_provider::Column::Channel.eq(channel))
        .filter(entity::notification_provider::Column::Enabled.eq(true))
        .order_by_desc(entity::notification_provider::Column::IsDefault)
        .order_by_asc(entity::notification_provider::Column::CreatedAt)
        .one(db)
        .await
        .ok()
        .flatten()
}

/// The provider a job routes through: an explicit `provider_id` in the payload
/// (the per-provider test button) wins over the channel default.
async fn provider_for_job(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    channel: &str,
    payload: &serde_json::Value,
) -> Option<entity::notification_provider::Model> {
    if let Some(pid) = payload
        .get("provider_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        if let Ok(Some(p)) = NotificationProvider::find_by_id(pid)
            .filter(entity::notification_provider::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
        {
            if p.channel == channel {
                return Some(p);
            }
        }
    }
    default_provider(db, tenant_id, channel).await
}

// ---------------------------------------------------------------------------
// Job handling
// ---------------------------------------------------------------------------

/// The natural de-duplication key for a send, when the payload carries enough
/// context to build one (`owner_type` + `owner_id`, plus an optional
/// `trigger`). Legacy payloads without owner context fall back to per-job
/// dedup via `notification.background_job_id`. User-directed channels get a
/// per-user key so a broadcast to N users isn't self-deduping.
fn idempotency_key(payload: &serde_json::Value, template: &str, channel: &str) -> Option<String> {
    let owner_type = payload.get("owner_type")?.as_str()?;
    let owner_id = payload.get("owner_id")?.as_str()?;
    let trigger = payload
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let user_suffix = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .map(|u| format!(":{u}"))
        .unwrap_or_default();
    Some(format!(
        "{channel}:{template}:{owner_type}:{owner_id}:{trigger}{user_suffix}"
    ))
}

/// Advance one `auto_email` / `auto_sms` / `auto_push` / `auto_chat` job:
/// render, deliver via the tenant's configured provider (or the simulated
/// fallback), persist the `notification` row, audit, and map provider errors
/// onto the retry budget. Called from the integrations module's `handle_job`.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let channel = match job.kind.as_str() {
        "auto_sms" => "sms",
        "auto_push" => "push",
        "auto_chat" => "chat",
        _ => "email",
    };
    let Some(template) = job.payload.get("template").and_then(|v| v.as_str()) else {
        return JobOutcome::failed("notification payload missing 'template'");
    };

    // Per-channel addressing.
    let user_id = job
        .payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let provider_row = provider_for_job(db, job.tenant_id, channel, &job.payload).await;
    let to: String = match channel {
        "push" => {
            if user_id.is_none() {
                return JobOutcome::failed("push payload missing 'user_id'");
            }
            job.payload
                .get("to")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| user_id.unwrap().to_string())
        }
        "chat" => match &provider_row {
            Some(p) => p.kind.clone(),
            // Chat is opt-in: no provider means nothing to deliver.
            None => {
                return JobOutcome::completed(json!({
                    "skipped": true,
                    "reason": "no chat provider configured",
                }))
            }
        },
        _ => match job.payload.get("to").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return JobOutcome::failed("notification payload missing 'to'"),
        },
    };

    // STOP always wins: a number that texted STOP gets nothing until START.
    let sms_message_id = job
        .payload
        .get("sms_message_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    if channel == "sms" && crate::texts::is_opted_out(db, job.tenant_id, &to).await {
        if let Some(mid) = sms_message_id {
            let _ = crate::texts::set_message_status(
                db,
                mid,
                "blocked",
                None,
                Some("this number texted STOP".into()),
            )
            .await;
        }
        return JobOutcome::completed(json!({
            "skipped": true,
            "reason": "opted_out",
            "channel": "sms",
            "template": template,
        }));
    }

    if channel == "sms" {
        // Marketing texts need their own consent on top of not having stopped.
        if job.payload.get("marketing").and_then(|v| v.as_bool()) == Some(true) {
            let consented = match crate::texts::normalize_phone(&to) {
                Some(p) => matches!(
                    crate::texts::find_thread(db, job.tenant_id, &p).await,
                    Ok(Some(t)) if t.marketing_opt_in_at.is_some()
                ),
                None => false,
            };
            if !consented {
                return JobOutcome::completed(json!({
                    "skipped": true,
                    "reason": "no_marketing_consent",
                    "channel": "sms",
                    "template": template,
                }));
            }
        }
        // Quiet hours: automatic texts wait for the morning. A text staff
        // typed (it has a message row already) goes when they send it.
        if sms_message_id.is_none() && !crate::text_auto::QUIET_EXEMPT.contains(&template) {
            if let Some(wait) = crate::text_auto::quiet_wait(db, job.tenant_id).await {
                let mut out = JobOutcome::reschedule("pending", wait);
                out.result = Some(json!({ "deferred": "quiet hours", "seconds": wait }));
                return out;
            }
        }
    }

    // Idempotency: has this natural trigger already sent (or is it in flight on
    // another job)?
    let idem = idempotency_key(&job.payload, template, channel);
    if let Some(key) = &idem {
        let existing = Notification::find()
            .filter(entity::notification::Column::TenantId.eq(job.tenant_id))
            .filter(entity::notification::Column::IdempotencyKey.eq(key.clone()))
            .one(db)
            .await;
        if let Ok(Some(n)) = existing {
            if n.background_job_id != Some(job.id) && n.status != "failed" {
                return JobOutcome::completed(json!({
                    "deduped": true,
                    "notification_id": n.id,
                }));
            }
        }
    }

    let (company, overrides) = tenant_context(db, job.tenant_id).await;
    let pairs = build_vars(
        &to,
        &company,
        template,
        job.payload.get("vars").and_then(|v| v.as_object()),
    );
    let vars: HashMap<&str, String> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    // The recipient's language: the job's own, else what's on file for the
    // address (a push or in-app message goes by the user's email).
    let lang = match job
        .payload
        .get("lang")
        .and_then(|v| v.as_str())
        .and_then(crate::language::parse)
    {
        Some(l) => l,
        None => crate::language::for_contact(db, job.tenant_id, &to).await,
    };
    let Some(rendered) = render(&overrides, channel, template, &vars, lang) else {
        return JobOutcome::failed(format!(
            "unknown notification template '{template}' (no platform default, no tenant override)"
        ));
    };

    // Ensure the notification row exists (find by job first so a retry updates
    // rather than duplicates).
    let row = match ensure_row(db, job, channel, template, &to, idem.as_deref(), user_id).await {
        Ok(r) => r,
        Err(e) => return JobOutcome::retry(providers::backoff(job.attempts), e.to_string()),
    };

    // Deliver through the provider framework (#16), routed to the tenant's
    // configured provider when one exists.
    let ctx = ProviderCtx::new(db, job.tenant_id);
    let req = MessageRequest {
        to: to.clone(),
        subject: rendered.subject.clone(),
        body: rendered.body.clone(),
    };
    let outcome: Result<MessageResponse, JobOutcome> = match channel {
        "sms" => match provider_row {
            Some(p) => providers::run(&SmsDelivery { row: p }, &ctx, job, &req).await,
            None => providers::run(&SimulatedSms, &ctx, job, &req).await,
        },
        "chat" => {
            let p = provider_row.expect("chat handled above when unconfigured");
            providers::run(&ChatDelivery { row: p }, &ctx, job, &req).await
        }
        "push" => {
            let push_req = PushRequest {
                user_id: user_id.expect("push validated above"),
                title: rendered.subject.clone().unwrap_or_else(|| company.clone()),
                body: rendered.body.clone(),
            };
            providers::run(&PushDelivery, &ctx, job, &push_req).await
        }
        _ => match provider_row {
            Some(p) => providers::run(&EmailDelivery { row: p }, &ctx, job, &req).await,
            None => providers::run(&SimulatedEmail, &ctx, job, &req).await,
        },
    };

    // File every text in its conversation so the office sees the whole thread.
    if channel == "sms" {
        let (status, provider_id, error) = match &outcome {
            Ok(resp) => ("sent", Some(resp.provider_message_id.clone()), None),
            Err(o) if o.status == "failed" => ("failed", None, o.error.clone()),
            // A retry is still in flight — record it once it settles.
            Err(_) => ("", None, None),
        };
        if !status.is_empty() {
            let filed = match sms_message_id {
                Some(mid) => crate::texts::set_message_status(db, mid, status, provider_id, error)
                    .await
                    .map(|_| ()),
                None => {
                    // Sign-in links never sit in the shared inbox.
                    let body = if template == "account_invite" || template == "password_reset" {
                        "(sign-in link sent — hidden)".to_string()
                    } else {
                        rendered.body.clone()
                    };
                    crate::texts::record_outbound(
                        db,
                        job.tenant_id,
                        crate::texts::Outbound {
                            to: &to,
                            body: &body,
                            status,
                            provider_message_id: provider_id,
                            template_key: Some(template.to_string()),
                            sent_by_user_id: None,
                            error,
                        },
                    )
                    .await
                    .map(|_| ())
                }
            };
            if let Err(e) = filed {
                tracing::warn!("couldn't file the text in its thread: {e}");
            }
        }
    }

    match outcome {
        Ok(resp) => {
            let notification_id = row.id;
            let mut am: entity::notification::ActiveModel = row.into();
            am.status = Set("sent".into());
            am.provider_message_id = Set(Some(resp.provider_message_id.clone()));
            am.subject = Set(rendered.subject);
            am.body = Set(Some(rendered.body));
            am.last_error = Set(None);
            am.updated_at = Set(Utc::now().into());
            if let Err(e) = am.update(db).await {
                tracing::error!("failed to persist sent notification: {e}");
            }

            // Audit the send: template + channel + status, never the rendered
            // body (it may carry PII).
            crate::audit::record(
                db,
                None,
                crate::audit::actions::NOTIFICATION_SEND,
                Some("notification"),
                Some(notification_id.to_string()),
                Some(job.tenant_id),
                Some(json!({
                    "template": template,
                    "channel": channel,
                    "status": "sent",
                })),
            )
            .await;

            JobOutcome::completed(json!({
                "sent": true,
                "channel": channel,
                "template": template,
                "notification_id": notification_id,
                "provider_message_id": resp.provider_message_id,
                "sent_at": Utc::now().to_rfc3339(),
            }))
        }
        Err(job_outcome) => {
            // Persist the delivery state on the notification row; terminal
            // failures are audited like sends.
            let terminal = job_outcome.status == "failed";
            let notification_id = row.id;
            let mut am: entity::notification::ActiveModel = row.into();
            if terminal {
                am.status = Set("failed".into());
            }
            am.last_error = Set(job_outcome.error.clone());
            am.updated_at = Set(Utc::now().into());
            if let Err(e) = am.update(db).await {
                tracing::error!("failed to persist notification delivery state: {e}");
            }
            if terminal {
                crate::audit::record(
                    db,
                    None,
                    crate::audit::actions::NOTIFICATION_SEND,
                    Some("notification"),
                    Some(notification_id.to_string()),
                    Some(job.tenant_id),
                    Some(json!({
                        "template": template,
                        "channel": channel,
                        "status": "failed",
                    })),
                )
                .await;
            }
            job_outcome
        }
    }
}

/// Load the notification row for this job, creating a `queued` one if this is
/// the first attempt.
#[allow(clippy::too_many_arguments)]
async fn ensure_row(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
    channel: &str,
    template: &str,
    to: &str,
    idem: Option<&str>,
    user_id: Option<Uuid>,
) -> anyhow::Result<entity::notification::Model> {
    if let Some(existing) = Notification::find()
        .filter(entity::notification::Column::BackgroundJobId.eq(job.id))
        .one(db)
        .await?
    {
        return Ok(existing);
    }
    // A previously-failed send with the same natural key is adopted (retried)
    // rather than duplicated — the unique index would reject a second insert.
    if let Some(key) = idem {
        if let Some(prior) = Notification::find()
            .filter(entity::notification::Column::TenantId.eq(job.tenant_id))
            .filter(entity::notification::Column::IdempotencyKey.eq(key))
            .one(db)
            .await?
        {
            let mut am: entity::notification::ActiveModel = prior.clone().into();
            am.background_job_id = Set(Some(job.id));
            am.updated_at = Set(Utc::now().into());
            return Ok(am.update(db).await?);
        }
    }
    let now = Utc::now();
    Ok(entity::notification::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(job.tenant_id),
        channel: Set(channel.into()),
        template_key: Set(template.into()),
        recipient: Set(to.into()),
        status: Set("queued".into()),
        provider_message_id: Set(None),
        subject: Set(None),
        body: Set(None),
        background_job_id: Set(Some(job.id)),
        idempotency_key: Set(idem.map(str::to_string)),
        user_id: Set(user_id),
        read_at: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

// ---------------------------------------------------------------------------
// In-app inbox + broadcast fan-out
// ---------------------------------------------------------------------------

/// Write one in-app notification directly (no provider, no job — the row *is*
/// the delivery). Silently skips duplicates on the idempotency key.
#[allow(clippy::too_many_arguments)]
pub async fn in_app(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user: &entity::user::Model,
    template: &str,
    vars_json: &serde_json::Value,
    owner: Option<(&str, Uuid)>,
    trigger: &str,
) -> Option<Uuid> {
    let (company, overrides) = tenant_context(db, tenant_id).await;
    let pairs = build_vars(&user.email, &company, template, vars_json.as_object());
    let vars: HashMap<&str, String> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let lang = crate::language::for_contact(db, tenant_id, &user.email).await;
    let rendered = render(&overrides, "in_app", template, &vars, lang)?;

    let idem =
        owner.map(|(otype, oid)| format!("in_app:{template}:{otype}:{oid}:{trigger}:{}", user.id));
    let now = Utc::now();
    let row = entity::notification::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        channel: Set("in_app".into()),
        template_key: Set(template.into()),
        recipient: Set(user.email.clone()),
        status: Set("sent".into()),
        provider_message_id: Set(None),
        subject: Set(rendered.subject),
        body: Set(Some(rendered.body)),
        background_job_id: Set(None),
        idempotency_key: Set(idem),
        user_id: Set(Some(user.id)),
        read_at: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    match row.insert(db).await {
        Ok(saved) => Some(saved.id),
        // A duplicate natural key means this trigger already notified the user.
        Err(e) => {
            tracing::debug!("in-app notification skipped (likely duplicate): {e}");
            None
        }
    }
}

/// Notify one specific person across every direct channel: email (always,
/// to `email`), plus the in-app inbox and a web-push job when the address
/// belongs to a platform account. This is the resident-update path — portal
/// users hear in their inbox/phone, plain-email tenants still get the mail.
#[allow(clippy::too_many_arguments)]
pub async fn notify_person(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    email: &str,
    template: &str,
    vars_json: serde_json::Value,
    owner: Option<(&str, Uuid)>,
    trigger: &str,
) {
    let email = email.trim();
    if email.is_empty() {
        return;
    }

    let mut payload = serde_json::Map::new();
    payload.insert("template".into(), json!(template));
    payload.insert("to".into(), json!(email));
    payload.insert("vars".into(), vars_json.clone());
    if let Some((otype, oid)) = owner {
        payload.insert("owner_type".into(), json!(otype));
        payload.insert("owner_id".into(), json!(oid.to_string()));
    }
    payload.insert("trigger".into(), json!(trigger));

    if let Err(e) = crate::scheduler::enqueue(
        db,
        tenant_id,
        "auto_email",
        serde_json::Value::Object(payload.clone()),
        0,
    )
    .await
    {
        tracing::error!("failed to enqueue auto_email: {e}");
    }

    // In-app + push only exist for someone with an account — in THIS tenant.
    // Emails are globally unique across the platform and lease contact
    // addresses are free-form staff input, so an unscoped match could hand
    // one org's ticket details to another org's user.
    let user = entity::prelude::User::find()
        .filter(entity::user::Column::TenantId.eq(tenant_id))
        .filter(entity::user::Column::Email.eq(email.to_lowercase()))
        .one(db)
        .await
        .ok()
        .flatten();
    if let Some(user) = user {
        in_app(db, tenant_id, &user, template, &vars_json, owner, trigger).await;
        payload.insert("user_id".into(), json!(user.id.to_string()));
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_push",
            serde_json::Value::Object(payload),
            0,
        )
        .await
        {
            tracing::error!("failed to enqueue auto_push: {e}");
        }
    }
}

/// Fan a tenant event out to every staff member holding `permission_key`:
/// an in-app inbox entry each (written now), a Web Push job each, and one
/// chat message when a chat provider is configured. This is the "integrated
/// notifications" path real events call. `exclude_user` skips the actor who
/// caused the event (no point notifying yourself).
#[allow(clippy::too_many_arguments)]
pub async fn notify_staff(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    permission_key: &str,
    template: &str,
    vars_json: serde_json::Value,
    owner: Option<(&str, Uuid)>,
    trigger: &str,
    exclude_user: Option<Uuid>,
) {
    let users = match staff_with_permission(db, tenant_id, permission_key).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("notify_staff recipient lookup failed: {e}");
            return;
        }
    };
    let users: Vec<entity::user::Model> = users
        .into_iter()
        .filter(|u| Some(u.id) != exclude_user)
        .collect();

    let owner_fields = |payload: &mut serde_json::Map<String, serde_json::Value>| {
        if let Some((otype, oid)) = owner {
            payload.insert("owner_type".into(), json!(otype));
            payload.insert("owner_id".into(), json!(oid.to_string()));
        }
        payload.insert("trigger".into(), json!(trigger));
    };

    for user in &users {
        // Inbox entry, immediately visible.
        in_app(db, tenant_id, user, template, &vars_json, owner, trigger).await;

        // Web push riding the durable queue, one job per user so retries are
        // isolated per recipient.
        let mut payload = serde_json::Map::new();
        payload.insert("template".into(), json!(template));
        payload.insert("to".into(), json!(user.email));
        payload.insert("user_id".into(), json!(user.id.to_string()));
        payload.insert("vars".into(), vars_json.clone());
        owner_fields(&mut payload);
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_push",
            serde_json::Value::Object(payload),
            0,
        )
        .await
        {
            tracing::error!("failed to enqueue auto_push: {e}");
        }
    }

    // One chat message per event (not per user); the handler no-ops if the
    // tenant never configured a chat provider.
    if default_provider(db, tenant_id, "chat").await.is_some() {
        let mut payload = serde_json::Map::new();
        payload.insert("template".into(), json!(template));
        payload.insert("vars".into(), vars_json.clone());
        owner_fields(&mut payload);
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_chat",
            serde_json::Value::Object(payload),
            0,
        )
        .await
        {
            tracing::error!("failed to enqueue auto_chat: {e}");
        }
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::NOTIFICATION_BROADCAST,
        owner.map(|(t, _)| t),
        owner.map(|(_, id)| id.to_string()),
        Some(tenant_id),
        Some(json!({
            "template": template,
            "trigger": trigger,
            "recipients": users.len(),
        })),
    )
    .await;
}

/// Active tenant users holding `permission_key` through any of their roles.
pub(crate) async fn staff_with_permission(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    permission_key: &str,
) -> anyhow::Result<Vec<entity::user::Model>> {
    let role_ids: Vec<Uuid> = entity::prelude::RolePermission::find()
        .filter(entity::role_permission::Column::Permission.eq(permission_key))
        .all(db)
        .await?
        .into_iter()
        .map(|rp| rp.role_id)
        .collect();
    if role_ids.is_empty() {
        return Ok(vec![]);
    }
    let mut user_ids: Vec<Uuid> = entity::prelude::UserRole::find()
        .filter(entity::user_role::Column::TenantId.eq(tenant_id))
        .filter(entity::user_role::Column::RoleId.is_in(role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|ur| ur.user_id)
        .collect();
    user_ids.sort();
    user_ids.dedup();
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    Ok(entity::prelude::User::find()
        .filter(entity::user::Column::Id.is_in(user_ids))
        .filter(entity::user::Column::Status.eq("active"))
        .all(db)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<(String, String)> {
        vec![
            ("recipient".into(), "taylor@example.com".into()),
            ("company".into(), "Northwind Property Group".into()),
            ("applicant".into(), "Casey Jones".into()),
        ]
    }

    fn map(pairs: &[(String, String)]) -> HashMap<&str, String> {
        pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()
    }

    #[test]
    fn renders_platform_default_email() {
        let pairs = vars();
        let r = render(
            &json!({}),
            "email",
            "application_approved",
            &map(&pairs),
            "en",
        )
        .unwrap();
        assert_eq!(
            r.subject.as_deref(),
            Some("Your application with Northwind Property Group has been approved")
        );
        assert!(r.body.contains("taylor@example.com"));
        assert!(r.body.contains("Northwind Property Group"));
    }

    #[test]
    fn renders_sms_variant() {
        let pairs = vars();
        let r = render(
            &json!({}),
            "sms",
            "application_approved",
            &map(&pairs),
            "en",
        )
        .unwrap();
        assert!(r.subject.is_none());
        assert!(r.body.starts_with("Northwind Property Group: good news"));
    }

    #[test]
    fn push_and_in_app_get_title_plus_short_body() {
        let pairs = vars();
        for channel in ["push", "in_app"] {
            let r = render(
                &json!({}),
                channel,
                "application_submitted",
                &map(&pairs),
                "en",
            )
            .unwrap();
            assert_eq!(
                r.subject.as_deref(),
                Some("New application from Casey Jones")
            );
            assert_eq!(
                r.body,
                "New application from Casey Jones — review it in the console."
            );
        }
    }

    #[test]
    fn chat_uses_short_body_without_subject() {
        let pairs = vars();
        let r = render(&json!({}), "chat", "test_notification", &map(&pairs), "en").unwrap();
        assert!(r.subject.is_none());
        assert!(r.body.contains("test notification"));
    }

    #[test]
    fn esign_templates_render_the_signing_link() {
        let pairs = vec![
            ("recipient".into(), "jordan@example.com".into()),
            ("company".into(), "Northwind Property Group".into()),
            ("signer".into(), "Jordan Renter".into()),
            (
                "document_title".into(),
                "Residential Lease Agreement".into(),
            ),
            (
                "sign_url".into(),
                "https://app.example.com/sign/tok123?tenant=northwind".into(),
            ),
        ];
        let r = render(&json!({}), "email", "esign_request", &map(&pairs), "en").unwrap();
        assert_eq!(
            r.subject.as_deref(),
            Some("Signature requested: Residential Lease Agreement")
        );
        assert!(r.body.contains("Hi Jordan Renter"));
        assert!(r
            .body
            .contains("https://app.example.com/sign/tok123?tenant=northwind"));

        let sms = render(&json!({}), "sms", "esign_request", &map(&pairs), "en").unwrap();
        assert!(sms.subject.is_none());
        assert!(sms
            .body
            .contains("Sign: https://app.example.com/sign/tok123?tenant=northwind"));

        // The reminder + completion variants also resolve.
        for key in [
            "esign_reminder",
            "esign_signed_staff",
            "esign_completed",
            "esign_completed_staff",
            "esign_declined_staff",
            "esign_voided",
        ] {
            assert!(
                render(&json!({}), "email", key, &map(&pairs), "en").is_some(),
                "template {key} missing"
            );
        }
    }

    #[test]
    fn tenant_override_wins_field_by_field() {
        let pairs = vars();
        let overrides = json!({
            "application_approved": { "subject": "Welcome home, {recipient}!" }
        });
        let r = render(
            &overrides,
            "email",
            "application_approved",
            &map(&pairs),
            "en",
        )
        .unwrap();
        // Overridden subject, default body.
        assert_eq!(
            r.subject.as_deref(),
            Some("Welcome home, taylor@example.com!")
        );
        assert!(r.body.contains("Great news"));

        // A bare-string override replaces the body wholesale.
        let plain = json!({ "application_approved": "Custom body for {recipient}." });
        let r = render(&plain, "email", "application_approved", &map(&pairs), "en").unwrap();
        assert_eq!(r.body, "Custom body for taylor@example.com.");
    }

    #[test]
    fn unknown_template_is_none_unless_overridden() {
        let empty: HashMap<&str, String> = HashMap::new();
        assert!(render(&json!({}), "email", "no_such_template", &empty, "en").is_none());
        let overrides = json!({ "no_such_template": "Hello!" });
        assert!(render(&overrides, "email", "no_such_template", &empty, "en").is_some());
    }

    #[test]
    fn idempotency_key_needs_owner_context() {
        let legacy = json!({ "template": "application_approved", "to": "a@b.c" });
        assert!(idempotency_key(&legacy, "application_approved", "email").is_none());

        let rich = json!({
            "template": "application_approved",
            "to": "a@b.c",
            "owner_type": "application",
            "owner_id": "6a1c…",
            "trigger": "approved",
        });
        assert_eq!(
            idempotency_key(&rich, "application_approved", "email").as_deref(),
            Some("email:application_approved:application:6a1c…:approved")
        );
    }

    #[test]
    fn idempotency_key_is_per_user_for_directed_channels() {
        let a = json!({
            "template": "application_submitted",
            "owner_type": "application", "owner_id": "X", "trigger": "submitted",
            "user_id": "user-a",
        });
        let b = json!({
            "template": "application_submitted",
            "owner_type": "application", "owner_id": "X", "trigger": "submitted",
            "user_id": "user-b",
        });
        let ka = idempotency_key(&a, "application_submitted", "push").unwrap();
        let kb = idempotency_key(&b, "application_submitted", "push").unwrap();
        assert_ne!(ka, kb, "a broadcast must not dedupe across recipients");
    }

    #[test]
    fn provider_channel_catalog_is_consistent() {
        for (channel, kinds) in PROVIDER_CHANNELS {
            assert!(!kinds.is_empty(), "channel {channel} has no kinds");
        }
        assert!(PROVIDER_CHANNELS.iter().any(|(c, _)| *c == "email"));
        assert!(PROVIDER_CHANNELS.iter().all(|(c, _)| *c != "push"));
    }

    fn placeholders(t: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let mut rest = t;
        while let Some(a) = rest.find('{') {
            let Some(b) = rest[a..].find('}') else { break };
            out.insert(rest[a + 1..a + b].to_string());
            rest = &rest[a + b + 1..];
        }
        out
    }

    #[test]
    fn spanish_templates_match_the_english_ones() {
        for es in es::ES_TEMPLATES {
            let en = DEFAULT_TEMPLATES
                .iter()
                .find(|t| t.key == es.key)
                .unwrap_or_else(|| panic!("{} has no English original", es.key));
            for (field, a, b) in [
                ("subject", en.subject, es.subject),
                ("body", en.body, es.body),
                ("sms", en.sms, es.sms),
            ] {
                assert_eq!(
                    placeholders(a),
                    placeholders(b),
                    "{}.{field}: the Spanish version must use the same placeholders",
                    es.key
                );
            }
        }
    }

    #[test]
    fn renders_in_the_recipients_language() {
        let pairs = vec![
            ("recipient".to_string(), "Ana".to_string()),
            ("company".to_string(), "Northwind".to_string()),
            ("amount".to_string(), "$1,850".to_string()),
            ("due_date".to_string(), "2026-11-01".to_string()),
            ("pay_url".to_string(), "https://x/pay".to_string()),
        ];
        let es = render(&json!({}), "email", "rent_due", &map(&pairs), "es").unwrap();
        assert!(es
            .subject
            .unwrap()
            .starts_with("El alquiler de $1,850 vence"));
        assert!(es.body.contains("Hola Ana"));
        let sms = render(&json!({}), "sms", "rent_due", &map(&pairs), "es").unwrap();
        assert!(sms.body.contains("Pagar: https://x/pay"));
        // No Spanish version: English.
        let staff = render(&json!({}), "email", "manager_digest", &map(&pairs), "es");
        let en = render(&json!({}), "email", "manager_digest", &map(&pairs), "en");
        assert_eq!(staff.map(|r| r.body), en.map(|r| r.body));
        // The workspace's Spanish override wins over the built-in Spanish.
        let overrides = json!({ "rent_due.es": { "sms": "Renta {amount}, {due_date}" } });
        let o = render(&overrides, "sms", "rent_due", &map(&pairs), "es").unwrap();
        assert_eq!(o.body, "Renta $1,850, 2026-11-01");
        // An English override doesn't leak into the Spanish message.
        let en_only = json!({ "rent_due": { "sms": "Rent now" } });
        let e = render(&en_only, "sms", "rent_due", &map(&pairs), "es").unwrap();
        assert!(e.body.starts_with("Northwind: el alquiler"));
    }
}
