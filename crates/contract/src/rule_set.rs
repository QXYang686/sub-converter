use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRuleSetRequest {
    pub name: String,
    /// `remote` | `inline` | `local`
    pub source_kind: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    /// `yaml` | `text` | `source-json` | `binary` | `adblock`
    pub content_format: String,
    #[serde(default)]
    pub interval: Option<i64>,
    /// `inline` 来源的初始内容。
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuleSetRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub category: Option<Option<String>>,
    #[serde(default)]
    pub content_format: Option<String>,
    #[serde(default, deserialize_with = "double_option_i64")]
    pub interval: Option<Option<i64>>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRuleSetPinnedRequest {
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetContentSummaryResponse {
    pub has_content: bool,
    pub byte_size: u32,
    pub rule_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetResponse {
    pub id: String,
    pub name: String,
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub content_format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<i64>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<RuleSetContentSummaryResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRuleSetResponse {
    /// `refreshed` | `notModified` | `skipped` | `failed`
    pub status: String,
}

fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

fn double_option_i64<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_distinguishes_absent_null_and_value() {
        let absent: UpdateRuleSetRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.category, None);
        assert_eq!(absent.interval, None);

        let cleared: UpdateRuleSetRequest =
            serde_json::from_str(r#"{"category": null, "interval": null}"#).unwrap();
        assert_eq!(cleared.category, Some(None));
        assert_eq!(cleared.interval, Some(None));

        let set: UpdateRuleSetRequest =
            serde_json::from_str(r#"{"category":"domain","interval":86400}"#).unwrap();
        assert_eq!(set.category, Some(Some("domain".to_string())));
        assert_eq!(set.interval, Some(Some(86_400)));
    }

    #[test]
    fn create_defaults_optional_fields() {
        let request: CreateRuleSetRequest = serde_json::from_str(
            r#"{"name":"reject","sourceKind":"remote","contentFormat":"yaml"}"#,
        )
        .unwrap();
        assert!(request.url.is_none());
        assert!(request.category.is_none());
        assert!(request.interval.is_none());
        assert!(request.content.is_none());
    }

    #[test]
    fn response_skips_empty_optional_fields() {
        let response = RuleSetResponse {
            id: "1".to_string(),
            name: "inline".to_string(),
            source_kind: "inline".to_string(),
            url: None,
            path: None,
            category: None,
            content_format: "text".to_string(),
            interval: None,
            enabled: true,
            created_at: 1,
            updated_at: 2,
            content: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("\"url\":"));
        assert!(!json.contains("\"content\":"));
    }
}
