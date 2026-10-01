//! **Back Office** module (Vantedge phase 2B) — expenses and mileage with
//! receipts, work-order and rehab costing, billing in-house work to owners
//! through accounts payable, and the payroll, profit and tax reports built from
//! the same hours and receipts.

use super::{ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::backoffice;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;

pub struct BackOfficeModule;

impl PlatformModule for BackOfficeModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "backoffice",
            name: "Back Office",
            description: "Expenses and mileage with receipts, work-order costing, billing \
                 in-house maintenance to owners, and payroll, profit and tax reports — all \
                 from the same hours and receipts.",
            permissions: &[
                Permission::ExpenseRead,
                Permission::ExpenseManage,
                Permission::PayrollRead,
            ],
            job_kinds: &[],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            backoffice::expenses::list,
            backoffice::expenses::create,
            backoffice::expenses::update,
            backoffice::expenses::remove,
            backoffice::expenses::reimburse,
            backoffice::expenses::add_receipt,
            backoffice::expenses::receipts,
            backoffice::expenses::mine,
            backoffice::expenses::add_mine,
            backoffice::expenses::edit_mine,
            backoffice::expenses::delete_mine,
            backoffice::costing::costs,
            backoffice::costing::bill_preview,
            backoffice::costing::bill_owner,
            backoffice::reports::payroll,
            backoffice::reports::payroll_export,
            backoffice::reports::timesheets_export,
            backoffice::reports::profit,
            backoffice::reports::profit_export,
            backoffice::reports::taxes,
            backoffice::reports::taxes_export,
            backoffice::reports::dashboard,
            backoffice::reports::cost_sheet,
            backoffice::gusto::gusto_status,
            backoffice::gusto::gusto_hours,
            backoffice::gusto::gusto_hours_csv,
            backoffice::gusto::gusto_push,
        ]
    }
}
