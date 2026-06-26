// ---------------------------------------------------------------------------
// Resolved grants on an authenticated principal
// ---------------------------------------------------------------------------

use super::permission::Permission;
use std::collections::HashSet;

/// Resolved permission set carried on an authenticated principal.
#[derive(Clone, Debug, Default)]
pub struct Grants(pub HashSet<String>);

impl Grants {
    pub fn from_iter<I: IntoIterator<Item = String>>(it: I) -> Self {
        Grants(it.into_iter().collect())
    }

    /// Whether the principal holds permission `p` (platform admins hold all).
    #[allow(dead_code)] // public convenience; handlers go through `require`/`has_key`.
    pub fn has(&self, p: Permission) -> bool {
        self.has_key(p.as_str())
    }

    /// String-keyed check, for dynamic/custom permissions not in [`Permission`].
    pub fn has_key(&self, key: &str) -> bool {
        self.0.contains(Permission::PlatformAdmin.as_str()) || self.0.contains(key)
    }
}
