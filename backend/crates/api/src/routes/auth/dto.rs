use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TokenResp {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UserResp {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// Primary tenant of the account (back-compat).
    pub tenant_id: Option<Uuid>,
    /// The workspace the current token is scoped to (`None` = Acre HQ / platform).
    pub active_tenant_id: Option<Uuid>,
    pub is_platform_staff: bool,
    pub permissions: Vec<String>,
    /// Every persona the user holds, across platform and tenants.
    pub memberships: Vec<MembershipSummary>,
    /// Workspaces the user can switch into (drives the workspace switcher).
    pub workspaces: Vec<WorkspaceSummary>,
}

/// One of a user's personas, with the owning workspace resolved for display.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MembershipSummary {
    pub scope: String,
    pub tenant_id: Option<Uuid>,
    pub tenant_slug: Option<String>,
    pub tenant_name: Option<String>,
    pub profile_type: String,
    pub title: Option<String>,
    pub status: String,
    pub is_primary: bool,
}

/// A workspace the user can operate in.
#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct WorkspaceSummary {
    /// `platform` (Acre HQ) or `tenant` (a client workspace).
    pub kind: String,
    pub tenant_id: Option<Uuid>,
    pub slug: Option<String>,
    pub name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RefreshReq {
    pub refresh_token: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SwitchReq {
    /// Target workspace; `null` selects the platform (Acre HQ) context.
    pub tenant_id: Option<Uuid>,
}

/// Response from a workspace switch — a fresh access token scoped to the chosen
/// workspace, with permissions re-resolved for it.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SwitchResp {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResp,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LogoutReq {
    pub refresh_token: String,
}

// ---------------------------------------------------------------------------
// MFA (TOTP) + federated login (issue #63)
// ---------------------------------------------------------------------------

/// The result of a password login: a full session, or — when the account has
/// TOTP MFA — a challenge that must be completed first. Untagged, so the
/// no-MFA path serializes exactly like [`TokenResp`] (backward compatible).
#[derive(Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum LoginResp {
    Token(Box<TokenResp>),
    Mfa(MfaChallengeResp),
}

/// A login step-up: the password/social factor passed, but the account has TOTP
/// MFA enabled, so a second factor is required before a session is issued.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MfaChallengeResp {
    /// Always `true` — present so clients can branch on the response shape.
    pub mfa_required: bool,
    /// Short-lived token binding this challenge to the user. Return it to
    /// `POST /auth/mfa/verify` with the current authenticator code.
    pub mfa_token: String,
}

/// Begin a TOTP MFA enrolment — the secret to store in an authenticator app.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TotpSetupResp {
    /// The base32 shared secret (for manual entry).
    pub secret: String,
    /// `otpauth://` URI the authenticator imports (usually via a QR code).
    pub otpauth_uri: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TotpCodeReq {
    /// The 6-digit code from the authenticator app.
    pub code: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct MfaStatusResp {
    pub enabled: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MfaVerifyReq {
    pub mfa_token: String,
    pub code: String,
}

/// Kick off a social-login flow — returns the provider authorize URL to send
/// the browser to.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct OauthStartReq {
    /// `login` (default) or `link` (attach this provider to the signed-in user).
    pub intent: Option<String>,
    /// Workspace slug to provision a first-time social signup into. Required for
    /// the `login` intent (a new user needs a home workspace).
    pub tenant: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct OauthStartResp {
    pub authorize_url: String,
    /// True when the hermetic sandbox provider is in use (no live credentials).
    pub sandbox: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OauthCallbackReq {
    pub code: String,
    pub state: String,
}

/// The result of an OAuth callback — a session, an MFA challenge, or (for the
/// `link` intent) a link confirmation. Exactly one payload field is set,
/// keyed by `outcome`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct OauthCallbackResp {
    /// `session` | `mfa` | `linked`.
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Box<TokenResp>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mfa: Option<MfaChallengeResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}
