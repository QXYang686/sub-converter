use std::collections::BTreeMap;

use serde_json::{Map, Value as JsonValue};
use serde_yaml::{Mapping, Value};

use super::yaml_json::yaml_to_json;
use crate::domain::ExtractedRuleProvider;

const RULE_PROVIDERS_KEY: &str = "rule-providers";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedProxy {
    pub protocol: Option<String>,
    pub name: Option<String>,
    pub server: Option<String>,
    pub port: Option<u16>,
    pub options_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedGroup {
    pub name: Option<String>,
    pub group_type: Option<String>,
    pub proxies_json: String,
    pub options_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedConfig {
    pub proxies: Vec<ExtractedProxy>,
    pub groups: Vec<ExtractedGroup>,
    pub rules: Vec<String>,
    pub rule_providers: Vec<ExtractedRuleProvider>,
    pub settings: JsonValue,
}

impl Default for ExtractedConfig {
    fn default() -> Self {
        Self {
            proxies: Vec::new(),
            groups: Vec::new(),
            rules: Vec::new(),
            rule_providers: Vec::new(),
            settings: JsonValue::Object(Map::new()),
        }
    }
}

impl ExtractedConfig {
    pub fn from_value(document: &Value) -> Self {
        let mapping = document.as_mapping();
        let proxies = sequence(mapping, "proxies")
            .map(|entries| entries.iter().map(extract_proxy).collect())
            .unwrap_or_default();
        let groups = sequence(mapping, "proxy-groups")
            .map(|entries| entries.iter().map(extract_group).collect())
            .unwrap_or_default();
        let rules = sequence(mapping, "rules")
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let settings = mapping.map(extract_settings).unwrap_or_default();
        let rule_providers = extract_rule_providers(&settings);

        Self {
            proxies,
            groups,
            rules,
            rule_providers,
            settings,
        }
    }

    pub fn proxy_count(&self) -> usize {
        self.proxies.len()
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn protocol_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for proxy in &self.proxies {
            let protocol = proxy.protocol.as_deref().unwrap_or("unknown");
            *counts.entry(protocol.to_string()).or_insert(0) += 1;
        }
        counts
    }

    pub fn rules_json(&self) -> String {
        serde_json::to_string(&self.rules).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn settings_json(&self) -> String {
        serde_json::to_string(&self.settings).unwrap_or_else(|_| "{}".to_string())
    }
}

fn sequence<'a>(mapping: Option<&'a Mapping>, key: &str) -> Option<&'a Vec<Value>> {
    mapping?.get(key)?.as_sequence()
}

fn extract_proxy(value: &Value) -> ExtractedProxy {
    let mapping = value.as_mapping();
    ExtractedProxy {
        protocol: string_field(mapping, "type"),
        name: string_field(mapping, "name"),
        server: string_field(mapping, "server"),
        port: mapping
            .and_then(|mapping| mapping.get("port"))
            .and_then(Value::as_u64)
            .filter(|port| *port <= u16::MAX as u64)
            .map(|port| port as u16),
        options_json: to_json(value),
    }
}

fn extract_group(value: &Value) -> ExtractedGroup {
    let mapping = value.as_mapping();
    let proxies = mapping
        .and_then(|mapping| mapping.get("proxies"))
        .map(yaml_to_json)
        .unwrap_or_else(|| JsonValue::Array(Vec::new()));
    ExtractedGroup {
        name: string_field(mapping, "name"),
        group_type: string_field(mapping, "type"),
        proxies_json: serde_json::to_string(&proxies).unwrap_or_else(|_| "[]".to_string()),
        options_json: to_json(value),
    }
}

fn extract_settings(mapping: &Mapping) -> JsonValue {
    let mut settings = Map::new();
    for (key, value) in mapping {
        let Some(key) = key.as_str() else {
            continue;
        };
        if matches!(key, "proxies" | "proxy-groups" | "rules") {
            continue;
        }
        settings.insert(key.to_string(), yaml_to_json(value));
    }
    JsonValue::Object(settings)
}

fn extract_rule_providers(settings: &JsonValue) -> Vec<ExtractedRuleProvider> {
    let Some(JsonValue::Object(providers)) = settings.get(RULE_PROVIDERS_KEY) else {
        return Vec::new();
    };
    providers
        .iter()
        .map(|(name, value)| ExtractedRuleProvider {
            name: name.clone(),
            provider_type: value
                .get("type")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            behavior: value
                .get("behavior")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            url: value
                .get("url")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            path: value
                .get("path")
                .and_then(JsonValue::as_str)
                .map(str::to_string),
            interval: value.get("interval").and_then(JsonValue::as_i64),
            options_json: serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
        })
        .collect()
}

fn string_field(mapping: Option<&Mapping>, key: &str) -> Option<String> {
    mapping?
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn to_json(value: &Value) -> String {
    serde_json::to_string(&yaml_to_json(value)).unwrap_or_else(|_| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
mixed-port: 7890
mode: rule
dns:
  enable: true
  nameserver:
    - 1.1.1.1
proxies:
  - name: VMess
    type: vmess
    server: v.example.com
    port: 443
    uuid: aaaa
  - name: SS
    type: ss
    server: s.example.com
    port: 8388
    cipher: aes-128-gcm
    password: pass
  - name: 没有 type 的条目
    server: mystery.example.com
    port: 1
  - not-a-mapping
proxy-groups:
  - name: PROXY
    type: select
    proxies: [VMess, SS]
  - name: AUTO
    type: url-test
    url: http://www.gstatic.com/generate_204
    interval: 300
    proxies: [VMess]
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
  - MATCH,PROXY
rule-providers:
  reject:
    type: http
    behavior: domain
    url: https://example.com/reject.yaml
"#;

    fn extracted() -> ExtractedConfig {
        let document: Value = serde_yaml::from_str(CONFIG).unwrap();
        ExtractedConfig::from_value(&document)
    }

    #[test]
    fn extracts_every_proxy_including_unknown() {
        let config = extracted();
        assert_eq!(config.proxy_count(), 4);
        assert_eq!(
            config.protocol_counts(),
            BTreeMap::from([
                ("ss".to_string(), 1),
                ("unknown".to_string(), 2),
                ("vmess".to_string(), 1),
            ])
        );
        let ss = &config.proxies[1];
        assert_eq!(ss.protocol.as_deref(), Some("ss"));
        assert_eq!(ss.server.as_deref(), Some("s.example.com"));
        assert_eq!(ss.port, Some(8388));
        let value: serde_json::Value = serde_json::from_str(&ss.options_json).unwrap();
        assert_eq!(value["cipher"], "aes-128-gcm");
        assert_eq!(value["password"], "pass");
    }

    #[test]
    fn extracts_groups_rules_and_settings() {
        let config = extracted();
        assert_eq!(config.group_count(), 2);
        assert_eq!(config.groups[1].group_type.as_deref(), Some("url-test"));
        let group_proxies: serde_json::Value =
            serde_json::from_str(&config.groups[1].proxies_json).unwrap();
        assert_eq!(group_proxies, serde_json::json!(["VMess"]));

        assert_eq!(config.rule_count(), 2);
        assert!(config.rules_json().contains("DOMAIN-SUFFIX"));

        let settings: serde_json::Value = serde_json::from_str(&config.settings_json()).unwrap();
        assert_eq!(settings["mixed-port"], 7890);
        assert_eq!(settings["dns"]["enable"], true);
        assert_eq!(settings["rule-providers"]["reject"]["behavior"], "domain");
        assert!(settings.get("proxies").is_none());
        assert!(settings.get("proxy-groups").is_none());
        assert!(settings.get("rules").is_none());
    }

    #[test]
    fn extracts_rule_provider_declarations() {
        let config = extracted();
        assert_eq!(config.rule_providers.len(), 1);
        let provider = &config.rule_providers[0];
        assert_eq!(provider.name, "reject");
        assert_eq!(provider.provider_type.as_deref(), Some("http"));
        assert_eq!(provider.behavior.as_deref(), Some("domain"));
        assert_eq!(
            provider.url.as_deref(),
            Some("https://example.com/reject.yaml")
        );
        assert!(provider.is_remote());
        let value: serde_json::Value = serde_json::from_str(&provider.options_json).unwrap();
        assert_eq!(value["behavior"], "domain");
        assert_eq!(value["url"], "https://example.com/reject.yaml");
    }

    #[test]
    fn skipped_when_no_rule_providers() {
        let config = extracted();
        let without: serde_json::Value = serde_json::json!({"proxies": []});
        assert!(
            ExtractedConfig::from_value(&serde_yaml::to_value(without).unwrap())
                .rule_providers
                .is_empty()
        );
        assert!(!config.rule_providers.is_empty());
    }

    #[test]
    fn extraction_tolerates_missing_sections() {
        let document: Value = serde_yaml::from_str("mode: rule").unwrap();
        let config = ExtractedConfig::from_value(&document);
        assert_eq!(config.proxy_count(), 0);
        assert_eq!(config.group_count(), 0);
        assert_eq!(config.rule_count(), 0);
        let settings: serde_json::Value = serde_json::from_str(&config.settings_json()).unwrap();
        assert_eq!(settings["mode"], "rule");
    }
}
