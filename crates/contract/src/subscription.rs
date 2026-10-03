use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSourceRequest {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSourceRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SourceSnapshotResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSnapshotResponse {
    pub fetched_at: Option<i64>,
    pub proxy_count: u32,
    pub group_count: u32,
    pub rule_count: u32,
    pub protocol_counts: Vec<ProtocolCountResponse>,
    pub upload: Option<i64>,
    pub download: Option<i64>,
    pub total: Option<i64>,
    pub expire: Option<i64>,
    pub update_interval: Option<i64>,
    pub provider_name: Option<String>,
    pub provider_url: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCountResponse {
    pub protocol: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleProviderResponse {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<i64>,
    pub rule_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<i64>,
    pub has_snapshot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePublicationRequest {
    pub name: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePublicationRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub expires_at: Option<Option<i64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPublicationSourcesRequest {
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationResponse {
    pub id: String,
    pub name: String,
    pub secret: String,
    pub enabled: bool,
    pub expires_at: Option<i64>,
    pub source_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_publication_distinguishes_absent_null_and_value() {
        let absent: UpdatePublicationRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.expires_at, None);

        let cleared: UpdatePublicationRequest =
            serde_json::from_str(r#"{"expiresAt": null}"#).unwrap();
        assert_eq!(cleared.expires_at, Some(None));

        let set: UpdatePublicationRequest =
            serde_json::from_str(r#"{"expiresAt": 1700000000}"#).unwrap();
        assert_eq!(set.expires_at, Some(Some(1_700_000_000)));
    }

    #[test]
    fn create_publication_defaults_to_empty_sources() {
        let request: CreatePublicationRequest =
            serde_json::from_str(r#"{"name":"我的订阅"}"#).unwrap();
        assert!(request.source_ids.is_empty());
        assert_eq!(request.expires_at, None);
    }

    #[test]
    fn source_response_snapshot_is_optional() {
        let json = r#"{
            "id": "1",
            "name": "A",
            "url": "https://a.example.com/sub",
            "enabled": true,
            "createdAt": 1,
            "updatedAt": 2
        }"#;
        let response: SourceResponse = serde_json::from_str(json).unwrap();
        assert!(response.snapshot.is_none());
        let serialized = serde_json::to_string(&response).unwrap();
        assert!(!serialized.contains("snapshot"));
    }
}
