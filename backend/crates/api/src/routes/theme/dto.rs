use serde::{Deserialize, Serialize};

#[derive(Serialize, schemars::JsonSchema)]
pub struct ThemeResp {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub accent_color: String,
    pub default_mode: String,
    pub legal_templates: serde_json::Value,
}

impl From<entity::theme::Model> for ThemeResp {
    fn from(t: entity::theme::Model) -> Self {
        ThemeResp {
            company_name: t.company_name,
            logo_url: t.logo_url,
            primary_color: t.primary_color,
            accent_color: t.accent_color,
            default_mode: t.default_mode,
            legal_templates: t.legal_templates,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateThemeReq {
    pub company_name: Option<String>,
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    pub default_mode: Option<String>,
    pub legal_templates: Option<serde_json::Value>,
}
