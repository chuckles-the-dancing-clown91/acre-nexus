//! Minimal permissive CORS fairing for local development so the Next.js frontend
//! (a different origin) can call the API. Tighten `Allow-Origin` for production.

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{options, Request, Response};

pub struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "CORS",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _req: &'r Request<'_>, res: &mut Response<'r>) {
        res.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        res.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "GET, POST, PATCH, PUT, DELETE, OPTIONS",
        ));
        res.set_header(Header::new(
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type, X-Tenant, X-Api-Key",
        ));
        res.set_header(Header::new("Access-Control-Max-Age", "86400"));
    }
}

/// Catch-all to answer CORS preflight requests with `204 No Content`.
#[options("/<_..>")]
pub fn preflight() -> rocket::http::Status {
    rocket::http::Status::NoContent
}
