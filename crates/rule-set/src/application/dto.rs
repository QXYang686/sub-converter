use crate::domain::{RuleSet, RuleSetContent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetContentSummary {
    pub has_content: bool,
    pub byte_size: u32,
    pub rule_count: u32,
    pub fetched_at: Option<i64>,
    pub body_hash: Option<String>,
    pub last_error: Option<String>,
    pub pinned: bool,
}

impl RuleSetContentSummary {
    pub fn from_content(content: &RuleSetContent) -> Self {
        let body = content.body();
        Self {
            has_content: body.is_some_and(|body| !body.is_empty()),
            byte_size: body.map(|body| body.len() as u32).unwrap_or(0),
            rule_count: content.rule_count(),
            fetched_at: (content.updated_at() > 0).then_some(content.updated_at()),
            body_hash: (!content.body_hash().is_empty()).then(|| content.body_hash().to_string()),
            last_error: content.last_error().map(str::to_string),
            pinned: content.pinned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSetView {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub category: Option<String>,
    pub content_format: String,
    pub interval: Option<i64>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub content: Option<RuleSetContentSummary>,
}

impl RuleSetView {
    pub fn from_parts(rule_set: &RuleSet, content: Option<&RuleSetContent>) -> Self {
        Self {
            id: rule_set.id().to_string(),
            name: rule_set.name().value().to_string(),
            source_kind: rule_set.source().kind().to_string(),
            url: rule_set
                .source()
                .url()
                .map(|url| url.value().to_string())
                .filter(|_| rule_set.source().is_remote()),
            path: rule_set
                .source()
                .path()
                .map(|path| path.value().to_string()),
            category: rule_set.category().map(|c| c.as_str().to_string()),
            content_format: rule_set.content_format().as_str().to_string(),
            interval: rule_set.interval().map(|interval| interval.seconds()),
            enabled: rule_set.enabled(),
            created_at: rule_set.created_at(),
            updated_at: rule_set.updated_at(),
            content: content.map(RuleSetContentSummary::from_content),
        }
    }
}
