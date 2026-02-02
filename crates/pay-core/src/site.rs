//! # Site Configuration
//!
//! Multi-tenant site configuration for lightning-cart.
//! Each site has its own branding, URLs, and statement descriptor.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a single tenant site
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    /// Unique site identifier (e.g., "chargegun", "spokenhope")
    pub id: String,

    /// Display name (e.g., "ChargeGun", "Spoken Hope")
    pub name: String,

    /// Primary domain (e.g., "chargegun.io", "spokenhope.care")
    pub domain: String,

    /// Statement descriptor suffix for bank statements
    /// Appears as "CHARGEGUN* SPOKENHOPE" on customer statements
    /// Max 22 chars total, so keep suffix under ~10 chars
    pub statement_descriptor_suffix: String,

    /// URL to redirect after successful payment
    pub success_url: String,

    /// URL to redirect if customer cancels
    pub cancel_url: String,

    /// Support email for this site
    #[serde(default)]
    pub support_email: Option<String>,

    /// Whether this site is active
    #[serde(default = "default_true")]
    pub active: bool,

    /// Additional site-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Site {
    /// Create a new site with required fields
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        domain: impl Into<String>,
    ) -> Self {
        let domain_str: String = domain.into();
        Self {
            id: id.into(),
            name: name.into(),
            domain: domain_str.clone(),
            statement_descriptor_suffix: "".to_string(),
            success_url: format!("https://{}/checkout/success", domain_str),
            cancel_url: format!("https://{}/checkout/cancel", domain_str),
            support_email: None,
            active: true,
            metadata: HashMap::new(),
        }
    }

    /// Builder: set statement descriptor suffix
    pub fn with_statement_descriptor(mut self, suffix: impl Into<String>) -> Self {
        self.statement_descriptor_suffix = suffix.into();
        self
    }

    /// Builder: set success URL
    pub fn with_success_url(mut self, url: impl Into<String>) -> Self {
        self.success_url = url.into();
        self
    }

    /// Builder: set cancel URL
    pub fn with_cancel_url(mut self, url: impl Into<String>) -> Self {
        self.cancel_url = url.into();
        self
    }

    /// Builder: set support email
    pub fn with_support_email(mut self, email: impl Into<String>) -> Self {
        self.support_email = Some(email.into());
        self
    }

    /// Builder: add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the success URL with session_id placeholder for Stripe
    pub fn success_url_with_session(&self) -> String {
        if self.success_url.contains('?') {
            format!("{}&session_id={{CHECKOUT_SESSION_ID}}", self.success_url)
        } else {
            format!("{}?session_id={{CHECKOUT_SESSION_ID}}", self.success_url)
        }
    }
}

/// Registry of all tenant sites
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteRegistry {
    /// List of sites from config
    #[serde(default)]
    pub sites: Vec<Site>,

    /// Default site ID (used when no site_id is specified)
    #[serde(skip)]
    default_site_id: Option<String>,
}

impl SiteRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            sites: Vec::new(),
            default_site_id: None,
        }
    }

    /// Create registry with a default site
    pub fn with_default(default_site_id: impl Into<String>) -> Self {
        Self {
            sites: Vec::new(),
            default_site_id: Some(default_site_id.into()),
        }
    }

    /// Add a site to the registry
    pub fn add(&mut self, site: Site) {
        self.sites.push(site);
    }

    /// Add a site with builder pattern
    pub fn with_site(mut self, site: Site) -> Self {
        self.add(site);
        self
    }

    /// Set the default site ID
    pub fn set_default(&mut self, site_id: impl Into<String>) {
        self.default_site_id = Some(site_id.into());
    }

    /// Get a site by ID
    pub fn get(&self, site_id: &str) -> Option<&Site> {
        self.sites.iter().find(|s| s.id == site_id && s.active)
    }

    /// Get the default site
    pub fn default_site(&self) -> Option<&Site> {
        self.default_site_id
            .as_ref()
            .and_then(|id| self.get(id))
            .or_else(|| self.sites.first())
    }

    /// Get site by ID or fall back to default
    pub fn get_or_default(&self, site_id: Option<&str>) -> Option<&Site> {
        match site_id {
            Some(id) => self.get(id).or_else(|| self.default_site()),
            None => self.default_site(),
        }
    }

    /// List all active sites
    pub fn active_sites(&self) -> impl Iterator<Item = &Site> {
        self.sites.iter().filter(|s| s.active)
    }

    /// Check if a site exists and is active
    pub fn has_site(&self, site_id: &str) -> bool {
        self.get(site_id).is_some()
    }

    /// Get all site IDs
    pub fn site_ids(&self) -> Vec<&str> {
        self.sites.iter().map(|s| s.id.as_str()).collect()
    }

    /// Get number of sites
    pub fn len(&self) -> usize {
        self.sites.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_creation() {
        let site = Site::new("spokenhope", "Spoken Hope", "spokenhope.care")
            .with_statement_descriptor("SPOKENHOPE")
            .with_support_email("info@spokenhope.care");

        assert_eq!(site.id, "spokenhope");
        assert_eq!(site.name, "Spoken Hope");
        assert_eq!(site.domain, "spokenhope.care");
        assert_eq!(site.statement_descriptor_suffix, "SPOKENHOPE");
        assert!(site.active);
    }

    #[test]
    fn test_success_url_with_session() {
        let site = Site::new("test", "Test", "test.com")
            .with_success_url("https://test.com/success");

        assert_eq!(
            site.success_url_with_session(),
            "https://test.com/success?session_id={CHECKOUT_SESSION_ID}"
        );

        let site2 = Site::new("test2", "Test2", "test2.com")
            .with_success_url("https://test2.com/success?ref=checkout");

        assert_eq!(
            site2.success_url_with_session(),
            "https://test2.com/success?ref=checkout&session_id={CHECKOUT_SESSION_ID}"
        );
    }

    #[test]
    fn test_site_registry() {
        let mut registry = SiteRegistry::with_default("chargegun");

        registry.add(Site::new("chargegun", "ChargeGun", "chargegun.io"));
        registry.add(Site::new("spokenhope", "Spoken Hope", "spokenhope.care"));

        assert_eq!(registry.len(), 2);
        assert!(registry.has_site("chargegun"));
        assert!(registry.has_site("spokenhope"));
        assert!(!registry.has_site("nonexistent"));

        let default = registry.default_site().unwrap();
        assert_eq!(default.id, "chargegun");

        let site = registry.get_or_default(Some("spokenhope")).unwrap();
        assert_eq!(site.id, "spokenhope");

        let fallback = registry.get_or_default(Some("nonexistent")).unwrap();
        assert_eq!(fallback.id, "chargegun");
    }
}
