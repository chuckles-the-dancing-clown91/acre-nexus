//! Document/email **templating** — Handlebars merge of an LLC's reusable template
//! body (`{{tenant_name}}`, `{{property_address}}`, `{{rent}}`, `{{llc_name}}`,
//! `{{signature_block}}`, …) with a render context assembled from the LLC's
//! branding plus the target lease / recipient.
//!
//! Output is plain text (no HTML escaping) — it feeds the PDF renderer and the
//! email body alike.

use handlebars::Handlebars;
use serde_json::Value;

/// Render a Handlebars `template` against `ctx`.
pub fn render(template: &str, ctx: &Value) -> anyhow::Result<String> {
    let mut hb = Handlebars::new();
    hb.register_escape_fn(handlebars::no_escape);
    // Don't fail the whole render on an unknown field — leave it blank.
    hb.set_strict_mode(false);
    Ok(hb.render_template(template, ctx)?)
}

/// A reasonable default **lease** body, used when an LLC has not authored its own
/// template yet. Placeholders are filled from the lease + LLC branding context.
pub const DEFAULT_LEASE_TEMPLATE: &str = "\
RESIDENTIAL LEASE AGREEMENT

This Lease Agreement (\"Agreement\") is entered into on {{today}} by and between \
{{llc_name}} (\"Landlord\"), a {{llc_state}} {{entity_type}}, and {{tenant_name}} \
(\"Tenant\").

1. PREMISES. Landlord leases to Tenant the premises located at {{property_address}}{{#if unit}}, Unit {{unit}}{{/if}}.

2. TERM. The lease term begins on {{start_date}} and ends on {{end_date}}.

3. RENT. Tenant shall pay rent of {{rent}} per month, due on the first day of each month.

4. SECURITY DEPOSIT. Tenant has paid a security deposit of {{deposit}}.

5. GOVERNING LAW. This Agreement is governed by the laws of the State of {{llc_state}}.

The parties have executed this Agreement as of the date first written above.";

/// A reasonable default **tenant letter** body.
pub const DEFAULT_LETTER_TEMPLATE: &str = "\
{{today}}

Dear {{tenant_name}},

This letter concerns the property located at {{property_address}}.

[Letter body]

Sincerely,";
