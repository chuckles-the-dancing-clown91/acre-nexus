//! **Accounting & payments** module — roadmap Phase 3 (issues #33–#39): the
//! double-entry general ledger + chart of accounts per legal entity, rent
//! collection through Stripe (sandbox-first) with saved methods + autopay,
//! the late-fee engine, Plaid bank feeds + reconciliation, owner payouts with
//! statements, and the dashboard finance series. On by default — collecting
//! rent is not an optional add-on for a PM platform.
//!
//! Owns the money job kinds: the self-rescheduling per-tenant `billing_cycle`
//! (receivables, late fees, autopay, feed refresh, snapshots), the per-charge
//! `payment_process`, the per-account `bank_feed_sync`, and the per-draw
//! `payout_execute`. Stripe/Plaid `webhook_event`s are dispatched here from
//! the integrations module.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{accounting, banking, payables, payments, payouts};
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct AccountingModule;

#[rocket::async_trait]
impl PlatformModule for AccountingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "accounting",
            name: "Accounting & Payments",
            description: "Double-entry ledger per legal entity, rent collection (cards/ACH \
                 with autopay), late fees, bank feeds + reconciliation, owner payouts with \
                 statements, and financial dashboards.",
            permissions: &[
                Permission::LedgerRead,
                Permission::LedgerManage,
                Permission::PaymentRead,
                Permission::PaymentManage,
                Permission::PayoutManage,
                Permission::PayableRead,
                Permission::PayableManage,
                Permission::PayableApprove,
            ],
            job_kinds: &[
                crate::billing::CYCLE_KIND,
                "payment_process",
                "bank_feed_sync",
                "payout_execute",
                crate::payables::PAY_JOB_KIND,
            ],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            // ledger: chart of accounts, journal, reports
            accounting::accounts::list_accounts,
            accounting::accounts::create_account,
            accounting::transactions::list_transactions,
            accounting::transactions::post_transaction,
            accounting::reports::trial_balance,
            accounting::reports::income_statement,
            accounting::reports::trust_reconciliation,
            accounting::series::finance_series,
            // payments: back-office visibility
            payments::list::list_payments,
            payments::methods::lease_methods,
            // renter portal: lease, methods, pay, autopay
            payments::portal::get_my_lease,
            payments::portal::add_method,
            payments::portal::remove_method,
            payments::portal::pay,
            payments::portal::set_autopay,
            payments::portal::cancel_autopay,
            // bank feeds + reconciliation
            banking::feed::list_all,
            banking::feed::link,
            banking::feed::sync,
            banking::feed::transactions,
            banking::feed::match_txn,
            banking::feed::ignore_txn,
            // owner payouts
            payouts::list::list_payouts,
            payouts::compute::compute_payout,
            payouts::execute::execute_payout,
            // accounts payable: vendor bills → approval → pay
            payables::list::list_payables,
            payables::get::get_payable,
            payables::create::create_payable,
            payables::update::update_payable,
            payables::submit::submit_payable,
            payables::approve::approve_payable,
            payables::reject::reject_payable,
            payables::void::void_payable,
            payables::pay::pay_payable,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            k if k == crate::billing::CYCLE_KIND => {
                Some(crate::billing::handle_cycle_job(ctx.db, ctx.job).await)
            }
            "payment_process" => Some(crate::payments::handle_process_job(ctx.db, ctx.job).await),
            "bank_feed_sync" => Some(crate::bankfeed::handle_sync_job(ctx.db, ctx.job).await),
            "payout_execute" => Some(crate::payouts::handle_payout_job(ctx.db, ctx.job).await),
            k if k == crate::payables::PAY_JOB_KIND => {
                Some(crate::payables::handle_pay_job(ctx.db, ctx.job).await)
            }
            _ => None,
        }
    }
}
