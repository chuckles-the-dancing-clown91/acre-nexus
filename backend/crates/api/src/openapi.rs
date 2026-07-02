//! OpenAPI plumbing.
//!
//! `rocket_okapi` auto-generates the OpenAPI spec from our `#[openapi]`-annotated
//! routes and `JsonSchema`-deriving DTOs. For that to work it also needs to know
//! how to document our custom request guards and error type — that is what this
//! module provides:
//!
//! * [`OpenApiFromRequest`] for the auth/tenant guards (so a route's security
//!   requirements show up in the docs), and
//! * [`OpenApiResponderInner`] for [`ApiError`] (so fallible routes document a
//!   response instead of failing to compile).
//!
//! The generated spec is served at `/openapi.json`, with interactive
//! **Swagger UI** at `/swagger-ui` and **RapiDoc** at `/rapidoc` (wired up in
//! [`crate::main`]).

use crate::auth::AuthUser;
use crate::db::RequestDb;
use crate::error::ApiError;
use crate::guards::{ClientIp, UserAgent};
use crate::routes::domains::resolve::HostHeader;
use crate::tenancy::{PublicTenant, TenantScope};
use crate::tokens::ApiPrincipal;
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::openapi3::{
    Object, Responses, SecurityRequirement, SecurityScheme, SecuritySchemeData,
};
use rocket_okapi::request::{OpenApiFromRequest, RequestHeaderInput};
use rocket_okapi::response::OpenApiResponderInner;
use rocket_okapi::Result;

/// Build a `bearer` HTTP security requirement. Both the JWT (human) and API-key
/// (vendor) guards authenticate via `Authorization: Bearer <token>`.
fn bearer_requirement(name: &str, description: &str) -> RequestHeaderInput {
    let scheme = SecurityScheme {
        description: Some(description.to_owned()),
        data: SecuritySchemeData::Http {
            scheme: "bearer".to_owned(),
            bearer_format: Some("JWT".to_owned()),
        },
        extensions: Object::default(),
    };
    let mut requirement = SecurityRequirement::default();
    requirement.insert(name.to_owned(), Vec::new());
    RequestHeaderInput::Security(name.to_owned(), scheme, requirement)
}

impl<'r> OpenApiFromRequest<'r> for AuthUser {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> Result<RequestHeaderInput> {
        Ok(bearer_requirement(
            "jwt",
            "JWT access token issued by POST /auth/login.",
        ))
    }
}

impl<'r> OpenApiFromRequest<'r> for ApiPrincipal {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> Result<RequestHeaderInput> {
        Ok(bearer_requirement(
            "api_key",
            "Scoped vendor API key (acre_live_…).",
        ))
    }
}

/// The tenant guards resolve the active tenant from the JWT or the `X-Tenant`
/// header; there is no separate documented parameter to add.
macro_rules! header_only_guard {
    ($guard:ty) => {
        impl<'r> OpenApiFromRequest<'r> for $guard {
            fn from_request_input(
                _gen: &mut OpenApiGenerator,
                _name: String,
                _required: bool,
            ) -> Result<RequestHeaderInput> {
                Ok(RequestHeaderInput::None)
            }
        }
    };
}

header_only_guard!(TenantScope);
header_only_guard!(PublicTenant);
header_only_guard!(HostHeader);
header_only_guard!(ClientIp);
header_only_guard!(UserAgent);
header_only_guard!(RequestDb);

impl OpenApiResponderInner for ApiError {
    fn responses(_gen: &mut OpenApiGenerator) -> Result<Responses> {
        // Errors share one JSON envelope `{ "error": { code, message } }`; the
        // exact status varies by case (400/401/403/404/409/500). We document the
        // contract in prose (docs/API.md) and keep the schema open here.
        Ok(Responses::default())
    }
}
