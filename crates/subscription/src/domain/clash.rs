mod extract;
mod yaml_json;

use std::collections::{HashMap, HashSet};

use serde_json::Value as JsonValue;
use serde_yaml::{Mapping, Value};
use thiserror::Error;

pub use extract::{ExtractedConfig, ExtractedGroup, ExtractedProxy};

use yaml_json::{json_to_yaml, yaml_to_json};

pub const PROXY_GROUP_NAME: &str = "PROXY";

#[derive(Debug, Clone, PartialEq)]
pub struct ClashProxy {
    protocol: String,
    identity: String,
    value: Value,
}

impl ClashProxy {
    fn from_value(value: &Value, index: usize) -> Option<Self> {
        let mapping = value.as_mapping()?;
        let protocol = mapping
            .get("type")?
            .as_str()
            .map(str::trim)
            .filter(|protocol| !protocol.is_empty())?
            .to_string();

        let mut value = value.clone();
        ensure_name(&mut value, &protocol, index);
        let identity = identity_for(&protocol, &value);

        Some(Self {
            protocol,
            identity,
            value,
        })
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn name(&self) -> Option<&str> {
        self.value.as_mapping()?.get("name")?.as_str()
    }
}

fn ensure_name(value: &mut Value, protocol: &str, index: usize) {
    let Some(mapping) = value.as_mapping_mut() else {
        return;
    };
    let has_name = mapping
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.trim().is_empty());
    if has_name {
        return;
    }

    let server = mapping
        .get("server")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let port = mapping.get("port").and_then(Value::as_u64);
    let fallback = match (server.is_empty(), port) {
        (false, Some(port)) => format!("{server}:{port}"),
        (false, None) => server.to_string(),
        _ => format!("{protocol} {}", index + 1),
    };
    mapping.insert(Value::String("name".to_string()), Value::String(fallback));
}

fn identity_for(protocol: &str, value: &Value) -> String {
    let Some(mapping) = value.as_mapping() else {
        return protocol.to_string();
    };
    let mut canonical = Mapping::new();
    for (key, value) in mapping {
        if key.as_str() == Some("name") {
            continue;
        }
        canonical.insert(key.clone(), value.clone());
    }
    let serialized =
        serde_json::to_string(&yaml_to_json(&Value::Mapping(canonical))).unwrap_or_default();
    format!("{protocol}|{serialized}")
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedClash {
    proxies: Vec<ClashProxy>,
    skipped: usize,
}

impl ParsedClash {
    pub fn proxies(&self) -> &[ClashProxy] {
        &self.proxies
    }

    pub fn skipped(&self) -> usize {
        self.skipped
    }

    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClashError {
    #[error("invalid yaml document")]
    InvalidYaml,
    #[error("clash config has no proxies section")]
    MissingProxies,
}

pub fn document(body: &str) -> Result<Value, ClashError> {
    serde_yaml::from_str(body).map_err(|_| ClashError::InvalidYaml)
}

pub fn parse_value(document: &Value) -> ParsedClash {
    let mut parsed = ParsedClash::default();
    let Some(entries) = document
        .as_mapping()
        .and_then(|mapping| mapping.get("proxies"))
        .and_then(Value::as_sequence)
    else {
        return parsed;
    };

    for (index, entry) in entries.iter().enumerate() {
        match ClashProxy::from_value(entry, index) {
            Some(proxy) => parsed.proxies.push(proxy),
            None => parsed.skipped += 1,
        }
    }
    parsed
}

pub fn parse(body: &str) -> Result<ParsedClash, ClashError> {
    let document = document(body)?;
    let mapping = document.as_mapping().ok_or(ClashError::InvalidYaml)?;
    mapping
        .get("proxies")
        .and_then(Value::as_sequence)
        .ok_or(ClashError::MissingProxies)?;
    Ok(parse_value(&document))
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedConfig {
    pub proxies: Vec<Value>,
    pub groups: Vec<Value>,
    pub rules: Vec<String>,
    pub settings: Mapping,
}

impl Default for MergedConfig {
    fn default() -> Self {
        Self {
            proxies: Vec::new(),
            groups: Vec::new(),
            rules: Vec::new(),
            settings: Mapping::new(),
        }
    }
}

#[derive(Default)]
struct MergedGroups {
    values: Vec<Value>,
    names: HashMap<String, String>,
    proxy_group: Option<String>,
}

#[derive(Default)]
struct ProviderNames {
    rules: HashMap<String, String>,
    proxies: HashMap<String, String>,
}

const RULE_PROVIDERS_KEY: &str = "rule-providers";
const PROXY_PROVIDERS_KEY: &str = "proxy-providers";

pub fn merge_configs<'a, I>(configs: I) -> MergedConfig
where
    I: IntoIterator<Item = &'a ExtractedConfig>,
{
    let mut merged = MergedConfig::default();
    let mut proxy_identity_names: HashMap<String, String> = HashMap::new();
    let mut used_proxy_names: HashSet<String> = HashSet::new();
    let mut used_group_names: HashSet<String> = HashSet::new();
    let mut used_rule_providers: HashSet<String> = HashSet::new();
    let mut used_proxy_providers: HashSet<String> = HashSet::new();
    let mut seen_rules: HashSet<String> = HashSet::new();
    let mut proxy_group: Option<String> = None;

    for config in configs {
        let providers = merge_settings(
            config,
            &mut merged.settings,
            &mut used_rule_providers,
            &mut used_proxy_providers,
        );
        let proxy_names = merge_proxies(
            config,
            &mut proxy_identity_names,
            &mut used_proxy_names,
            &mut merged.proxies,
        );
        let groups = merge_groups(
            config,
            &proxy_names,
            &providers.proxies,
            &mut used_group_names,
        );
        if proxy_group.is_none() {
            proxy_group = groups.proxy_group.clone();
        }
        merged.groups.extend(groups.values);
        merge_rules(
            config,
            &proxy_names,
            &groups.names,
            &providers.rules,
            &mut seen_rules,
            &mut merged.rules,
        );
    }

    let fallback = proxy_group.unwrap_or_else(|| synthesize_proxy_group(&mut merged));
    merged.settings.insert(
        Value::String("mode".to_string()),
        Value::String("rule".to_string()),
    );
    merged.rules.push(format!("MATCH,{fallback}"));
    merged
}

pub fn render_config(config: &MergedConfig) -> String {
    let mut root = config.settings.clone();
    root.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(config.proxies.clone()),
    );
    root.insert(
        Value::String("proxy-groups".to_string()),
        Value::Sequence(config.groups.clone()),
    );
    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(config.rules.iter().cloned().map(Value::String).collect()),
    );

    serde_yaml::to_string(&Value::Mapping(root)).unwrap_or_default()
}

fn merge_proxies(
    config: &ExtractedConfig,
    identity_names: &mut HashMap<String, String>,
    used_names: &mut HashSet<String>,
    merged: &mut Vec<Value>,
) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for (index, extracted) in config.proxies.iter().enumerate() {
        let Some(value) = json_value(&extracted.options_json) else {
            continue;
        };
        let Some(proxy) = ClashProxy::from_value(&value, index) else {
            continue;
        };
        let original = proxy.name().unwrap_or_default().to_string();
        if let Some(existing) = identity_names.get(proxy.identity()) {
            names.insert(original, existing.clone());
            continue;
        }

        let mut value = proxy.value.clone();
        uniquify_name(&mut value, used_names);
        let final_name = mapping_name(&value).unwrap_or_default();
        identity_names.insert(proxy.identity().to_string(), final_name.clone());
        names.insert(original, final_name);
        merged.push(value);
    }
    names
}

fn merge_groups(
    config: &ExtractedConfig,
    proxy_names: &HashMap<String, String>,
    proxy_providers: &HashMap<String, String>,
    used_names: &mut HashSet<String>,
) -> MergedGroups {
    let mut groups = MergedGroups::default();
    for group in &config.groups {
        let Some(value) = json_value(&group.options_json) else {
            continue;
        };
        let Some(original) = mapping_name(&value) else {
            continue;
        };
        let final_name = unique_name(&original, used_names);
        let mut value = value;
        set_mapping_name(&mut value, &final_name);
        groups.names.insert(original.clone(), final_name.clone());
        if original == PROXY_GROUP_NAME && groups.proxy_group.is_none() {
            groups.proxy_group = Some(final_name);
        }
        groups.values.push(value);
    }

    for value in &mut groups.values {
        rewrite_group_references(value, proxy_names, &groups.names, proxy_providers);
    }
    groups
}

fn rewrite_group_references(
    value: &mut Value,
    proxy_names: &HashMap<String, String>,
    group_names: &HashMap<String, String>,
    proxy_providers: &HashMap<String, String>,
) {
    let Some(mapping) = value.as_mapping_mut() else {
        return;
    };
    if let Some(members) = mapping.get_mut("proxies").and_then(Value::as_sequence_mut) {
        for member in members.iter_mut() {
            let Some(name) = member.as_str().map(str::to_string) else {
                continue;
            };
            if let Some(final_name) = proxy_names.get(&name).or_else(|| group_names.get(&name)) {
                *member = Value::String(final_name.clone());
            }
        }
    }
    if let Some(providers) = mapping.get_mut("use").and_then(Value::as_sequence_mut) {
        for provider in providers.iter_mut() {
            let Some(name) = provider.as_str().map(str::to_string) else {
                continue;
            };
            if let Some(final_name) = proxy_providers.get(&name) {
                *provider = Value::String(final_name.clone());
            }
        }
    }
}

fn merge_rules(
    config: &ExtractedConfig,
    proxy_names: &HashMap<String, String>,
    group_names: &HashMap<String, String>,
    rule_providers: &HashMap<String, String>,
    seen: &mut HashSet<String>,
    merged: &mut Vec<String>,
) {
    for rule in &config.rules {
        let rule = rule.trim();
        if rule.is_empty() {
            continue;
        }
        let kind = rule.split_once(',').map(|(kind, _)| kind).unwrap_or(rule);
        if matches!(kind.trim().to_ascii_uppercase().as_str(), "MATCH" | "FINAL") {
            continue;
        }
        let rewritten = rewrite_rule_target(rule, proxy_names, group_names);
        let rewritten = rewrite_rule_set_payload(&rewritten, rule_providers);
        if seen.insert(rewritten.clone()) {
            merged.push(rewritten);
        }
    }
}

fn rewrite_rule_set_payload(rule: &str, rule_providers: &HashMap<String, String>) -> String {
    let mut parts: Vec<&str> = rule.split(',').collect();
    if parts.len() < 2 || !parts[0].trim().eq_ignore_ascii_case("RULE-SET") {
        return rule.to_string();
    }
    let payload = parts[1].trim();
    let Some(final_name) = rule_providers.get(payload) else {
        return rule.to_string();
    };
    parts[1] = final_name;
    parts.join(",")
}

fn rewrite_rule_target(
    rule: &str,
    proxy_names: &HashMap<String, String>,
    group_names: &HashMap<String, String>,
) -> String {
    let mut parts: Vec<&str> = rule.split(',').collect();
    if parts.len() < 2 {
        return rule.to_string();
    }
    let mut target_index = parts.len() - 1;
    if parts[target_index].trim() == "no-resolve" {
        if target_index == 0 {
            return rule.to_string();
        }
        target_index -= 1;
    }
    let target = parts[target_index].trim();
    let Some(final_name) = proxy_names.get(target).or_else(|| group_names.get(target)) else {
        return rule.to_string();
    };
    parts[target_index] = final_name;
    parts.join(",")
}

fn merge_settings(
    config: &ExtractedConfig,
    settings: &mut Mapping,
    used_rule_providers: &mut HashSet<String>,
    used_proxy_providers: &mut HashSet<String>,
) -> ProviderNames {
    let mut providers = ProviderNames::default();
    let JsonValue::Object(object) = &config.settings else {
        return providers;
    };
    for (key, value) in object {
        match key.as_str() {
            "proxies" | "proxy-groups" | "rules" => continue,
            RULE_PROVIDERS_KEY => merge_provider_map(
                RULE_PROVIDERS_KEY,
                value,
                settings,
                used_rule_providers,
                &mut providers.rules,
            ),
            PROXY_PROVIDERS_KEY => merge_provider_map(
                PROXY_PROVIDERS_KEY,
                value,
                settings,
                used_proxy_providers,
                &mut providers.proxies,
            ),
            _ => {
                let key = Value::String(key.clone());
                if !settings.contains_key(&key) {
                    settings.insert(key, json_to_yaml(value));
                }
            }
        }
    }
    providers
}

fn merge_provider_map(
    settings_key: &str,
    providers: &JsonValue,
    settings: &mut Mapping,
    used: &mut HashSet<String>,
    names: &mut HashMap<String, String>,
) {
    let JsonValue::Object(providers) = providers else {
        return;
    };
    let key = Value::String(settings_key.to_string());
    if !settings.contains_key(&key) {
        settings.insert(key.clone(), Value::Mapping(Mapping::new()));
    }
    let Some(target) = settings.get_mut(&key).and_then(Value::as_mapping_mut) else {
        return;
    };
    for (name, provider) in providers {
        let final_name = unique_name(name, used);
        if final_name != *name {
            names.insert(name.clone(), final_name.clone());
        }
        target.insert(Value::String(final_name), json_to_yaml(provider));
    }
}

fn synthesize_proxy_group(merged: &mut MergedConfig) -> String {
    let mut members: Vec<Value> = Vec::new();
    if merged.groups.is_empty() {
        members.extend(merged.proxies.iter().filter_map(mapping_name_value));
    } else {
        members.extend(merged.groups.iter().filter_map(mapping_name_value));
        members.extend(merged.proxies.iter().filter_map(mapping_name_value));
    }
    members.push(Value::String("DIRECT".to_string()));

    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(PROXY_GROUP_NAME.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String("select".to_string()),
    );
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(members),
    );
    merged.groups.insert(0, Value::Mapping(group));
    PROXY_GROUP_NAME.to_string()
}

fn uniquify_name(value: &mut Value, used: &mut HashSet<String>) {
    let Some(mapping) = value.as_mapping_mut() else {
        return;
    };
    let Some(original) = mapping
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return;
    };
    let final_name = unique_name(&original, used);
    if final_name != original {
        mapping.insert(Value::String("name".to_string()), Value::String(final_name));
    }
}

fn unique_name(original: &str, used: &mut HashSet<String>) -> String {
    if used.insert(original.to_string()) {
        return original.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{original} {suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn json_value(raw: &str) -> Option<Value> {
    serde_json::from_str::<JsonValue>(raw)
        .ok()
        .map(|value| json_to_yaml(&value))
}

fn mapping_name(value: &Value) -> Option<String> {
    value
        .as_mapping()?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

fn mapping_name_value(value: &Value) -> Option<Value> {
    value
        .as_mapping()?
        .get("name")
        .filter(|name| name.as_str().is_some())
        .cloned()
}

fn set_mapping_name(value: &mut Value, name: &str) {
    if let Some(mapping) = value.as_mapping_mut() {
        mapping.insert(
            Value::String("name".to_string()),
            Value::String(name.to_string()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_A: &str = r#"
mixed-port: 7890
mode: rule
proxies:
  - name: 香港 01
    type: vmess
    server: hk1.example.com
    port: 443
    uuid: 11111111-1111-1111-1111-111111111111
    alterId: 0
    cipher: auto
    tls: true
  - name: 日本 01
    type: anytls
    server: jp1.example.com
    port: 8443
    password: anytls-secret
    sni: jp1.example.com
    skip-cert-verify: true
  - name: 被保留的 ss
    type: ss
    server: ss.example.com
    port: 8388
    cipher: aes-128-gcm
    password: ss-secret
  - not-a-mapping-entry
proxy-groups:
  - name: PROXY
    type: select
    proxies: [香港 01]
rules:
  - MATCH,PROXY
"#;

    const SOURCE_B: &str = r#"
proxies:
  - name: 香港 01
    type: vmess
    server: hk1.example.com
    port: 443
    uuid: 11111111-1111-1111-1111-111111111111
    alterId: 0
    cipher: auto
    tls: true
  - name: 香港 01
    type: hysteria2
    server: hy2.example.com
    port: 443
    password: hy2-secret
    obfs: salamander
    obfs-password: obfs-secret
  - name: 新加坡 01
    type: hysteria2
    server: sg.example.com
    port: 443
    password: hy2-secret-2
"#;

    fn parsed(source: &str) -> ParsedClash {
        parse(source).unwrap()
    }

    #[test]
    fn parse_keeps_all_protocols_and_skips_invalid_entries() {
        let result = parsed(SOURCE_A);
        assert_eq!(result.proxies().len(), 3);
        assert_eq!(result.skipped(), 1);
        assert_eq!(result.proxies()[0].protocol(), "vmess");
        assert_eq!(result.proxies()[1].protocol(), "anytls");
        assert_eq!(result.proxies()[2].protocol(), "ss");
        assert_eq!(result.proxies()[0].name(), Some("香港 01"));
    }

    #[test]
    fn parse_rejects_invalid_or_proxyless_documents() {
        assert_eq!(parse("not: [valid"), Err(ClashError::InvalidYaml));
        assert_eq!(parse("mode: rule"), Err(ClashError::MissingProxies));
        assert_eq!(parse("- just\n- a\n- list"), Err(ClashError::InvalidYaml));
    }

    #[test]
    fn parse_synthesizes_missing_names() {
        let result = parsed(
            r#"
proxies:
  - type: trojan
    server: trojan.example.com
    port: 443
    password: secret
  - type: ss
"#,
        );
        assert_eq!(result.proxies().len(), 2);
        assert_eq!(result.proxies()[0].name(), Some("trojan.example.com:443"));
        assert_eq!(result.proxies()[1].name(), Some("ss 2"));
    }

    #[test]
    fn identity_ignores_names_but_distinguishes_fields() {
        let first = ClashProxy::from_value(
            &serde_yaml::from_str(
                "type: ss\nname: A\nserver: s.example.com\nport: 8388\ncipher: aes-128-gcm\npassword: x",
            )
            .unwrap(),
            0,
        )
        .unwrap();
        let second = ClashProxy::from_value(
            &serde_yaml::from_str(
                "type: ss\nname: B\nserver: s.example.com\nport: 8388\ncipher: aes-128-gcm\npassword: x",
            )
            .unwrap(),
            0,
        )
        .unwrap();
        let third = ClashProxy::from_value(
            &serde_yaml::from_str(
                "type: ss\nname: C\nserver: s.example.com\nport: 8388\ncipher: aes-128-gcm\npassword: y",
            )
            .unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_ne!(first.identity(), third.identity());
    }

    fn config(source: &str) -> ExtractedConfig {
        let document: Value = serde_yaml::from_str(source).unwrap();
        ExtractedConfig::from_value(&document)
    }

    fn value_names(values: &[Value]) -> Vec<&str> {
        values
            .iter()
            .map(|value| {
                value
                    .as_mapping()
                    .unwrap()
                    .get("name")
                    .unwrap()
                    .as_str()
                    .unwrap()
            })
            .collect()
    }

    fn group_members(config: &MergedConfig, group_name: &str) -> Vec<String> {
        config
            .groups
            .iter()
            .find(|group| {
                group
                    .as_mapping()
                    .and_then(|mapping| mapping.get("name"))
                    .and_then(Value::as_str)
                    == Some(group_name)
            })
            .and_then(|group| group.as_mapping()?.get("proxies")?.as_sequence())
            .unwrap()
            .iter()
            .map(|member| member.as_str().unwrap().to_string())
            .collect()
    }

    fn group_uses(config: &MergedConfig, group_name: &str) -> Vec<String> {
        config
            .groups
            .iter()
            .find(|group| {
                group
                    .as_mapping()
                    .and_then(|mapping| mapping.get("name"))
                    .and_then(Value::as_str)
                    == Some(group_name)
            })
            .and_then(|group| group.as_mapping()?.get("use")?.as_sequence())
            .map(|uses| {
                uses.iter()
                    .map(|provider| provider.as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn round_trip(output: &str) -> Mapping {
        serde_yaml::from_str::<Value>(output)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone()
    }

    #[test]
    fn merge_configs_dedupes_identity_and_renames_conflicts() {
        let source_a = config(SOURCE_A);
        let source_b = config(SOURCE_B);
        let merged = merge_configs([&source_a, &source_b]);

        assert_eq!(
            value_names(&merged.proxies),
            vec![
                "香港 01",
                "日本 01",
                "被保留的 ss",
                "香港 01 2",
                "新加坡 01"
            ],
            "duplicates across sources must be dropped and name conflicts suffixed"
        );
    }

    #[test]
    fn merge_configs_preserves_source_order() {
        let source_b = config(SOURCE_B);
        let source_a = config(SOURCE_A);
        let merged = merge_configs([&source_b, &source_a]);
        let first = merged.proxies[0].as_mapping().unwrap().get("name").unwrap();
        assert_eq!(first.as_str(), Some("香港 01"));
        assert_eq!(merged.proxies.len(), 5);
    }

    #[test]
    fn render_config_keeps_source_groups_and_rules() {
        let source = config(SOURCE_A);
        let merged = merge_configs([&source]);
        let root = round_trip(&render_config(&merged));

        assert_eq!(root.get("mode").unwrap().as_str(), Some("rule"));
        assert_eq!(root.get("proxies").unwrap().as_sequence().unwrap().len(), 3);
        let rules: Vec<&str> = root
            .get("rules")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(rules, vec!["MATCH,PROXY"]);
        let group_names = value_names(root.get("proxy-groups").unwrap().as_sequence().unwrap());
        assert_eq!(group_names, vec!["PROXY"]);
        assert_eq!(
            group_members(&merged, "PROXY"),
            vec!["香港 01".to_string()],
            "source groups are preserved instead of being rebuilt"
        );
    }

    #[test]
    fn merge_configs_without_groups_synthesizes_proxy_group() {
        let source = config(SOURCE_B);
        let merged = merge_configs([&source]);

        assert_eq!(
            value_names(&merged.groups),
            vec!["PROXY"],
            "a PROXY selector is synthesized when sources have no groups"
        );
        assert_eq!(
            group_members(&merged, "PROXY"),
            vec![
                "香港 01".to_string(),
                "香港 01 2".to_string(),
                "新加坡 01".to_string(),
                "DIRECT".to_string()
            ]
        );
        assert_eq!(merged.rules, vec!["MATCH,PROXY"]);
    }

    #[test]
    fn merge_configs_without_sources_yields_direct_fallback() {
        let merged = merge_configs(std::iter::empty::<&ExtractedConfig>());
        let root = round_trip(&render_config(&merged));

        assert!(root
            .get("proxies")
            .unwrap()
            .as_sequence()
            .unwrap()
            .is_empty());
        assert_eq!(group_members(&merged, "PROXY"), vec!["DIRECT".to_string()]);
        assert_eq!(merged.rules, vec!["MATCH,PROXY"]);
    }

    #[test]
    fn merge_configs_keeps_rules_settings_and_rewrites_targets() {
        const SOURCE: &str = r#"
mixed-port: 7890
mode: global
dns:
  enable: true
proxies:
  - name: 香港 01
    type: vmess
    server: hk.example.com
    port: 443
    uuid: aaaa
proxy-groups:
  - name: 自动
    type: url-test
    url: http://example.com
    interval: 300
    proxies: [香港 01]
  - name: 手动
    type: select
    proxies: [香港 01, 自动]
rules:
  - DOMAIN-SUFFIX,example.com,自动
  - GEOIP,CN,DIRECT,no-resolve
  - AND,((NETWORK,TCP),(DOMAIN,example.com)),手动
  - MATCH,手动
"#;
        let merged = merge_configs([&config(SOURCE)]);

        assert_eq!(
            merged.settings.get("mixed-port").unwrap().as_u64(),
            Some(7890)
        );
        assert_eq!(merged.settings.get("mode").unwrap().as_str(), Some("rule"));
        assert_eq!(
            merged
                .settings
                .get("dns")
                .and_then(|dns| dns.as_mapping())
                .and_then(|dns| dns.get("enable"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            merged.rules,
            vec![
                "DOMAIN-SUFFIX,example.com,自动",
                "GEOIP,CN,DIRECT,no-resolve",
                "AND,((NETWORK,TCP),(DOMAIN,example.com)),手动",
                "MATCH,PROXY",
            ],
            "source MATCH rules are dropped in favor of the merged fallback"
        );
        assert_eq!(
            value_names(&merged.groups),
            vec!["PROXY", "自动", "手动"],
            "a PROXY selector is prepended when sources have groups but no PROXY"
        );
        assert_eq!(
            group_members(&merged, "PROXY"),
            vec![
                "自动".to_string(),
                "手动".to_string(),
                "香港 01".to_string(),
                "DIRECT".to_string()
            ]
        );
    }

    #[test]
    fn merge_configs_renames_groups_and_rewrites_references() {
        const FIRST: &str = r#"
proxies:
  - name: A-1
    type: ss
    server: a.example.com
    port: 443
    cipher: aes-128-gcm
    password: a
proxy-groups:
  - name: AUTO
    type: select
    proxies: [A-1]
rules:
  - DOMAIN,first.example.com,AUTO
  - MATCH,AUTO
"#;
        const SECOND: &str = r#"
proxies:
  - name: B-1
    type: ss
    server: b.example.com
    port: 443
    cipher: aes-128-gcm
    password: b
proxy-groups:
  - name: AUTO
    type: select
    proxies: [B-1]
  - name: NESTED
    type: select
    proxies: [AUTO]
rules:
  - DOMAIN,second.example.com,AUTO
  - MATCH,AUTO
"#;
        let first = config(FIRST);
        let second = config(SECOND);
        let merged = merge_configs([&first, &second]);

        assert_eq!(
            value_names(&merged.groups),
            vec!["PROXY", "AUTO", "AUTO 2", "NESTED"]
        );
        assert_eq!(group_members(&merged, "AUTO"), vec!["A-1".to_string()]);
        assert_eq!(group_members(&merged, "AUTO 2"), vec!["B-1".to_string()]);
        assert_eq!(
            group_members(&merged, "NESTED"),
            vec!["AUTO 2".to_string()],
            "group references must follow renamed siblings"
        );
        assert_eq!(
            merged.rules,
            vec![
                "DOMAIN,first.example.com,AUTO",
                "DOMAIN,second.example.com,AUTO 2",
                "MATCH,PROXY",
            ]
        );
    }

    #[test]
    fn merge_configs_rewrites_references_to_deduped_proxies() {
        const FIRST: &str = r#"
proxies:
  - name: 香港 A
    type: ss
    server: shared.example.com
    port: 443
    cipher: aes-128-gcm
    password: same
"#;
        const SECOND: &str = r#"
proxies:
  - name: 香港 B
    type: ss
    server: shared.example.com
    port: 443
    cipher: aes-128-gcm
    password: same
proxy-groups:
  - name: B 组
    type: select
    proxies: [香港 B]
rules:
  - DOMAIN,b.example.com,香港 B
"#;
        let first = config(FIRST);
        let second = config(SECOND);
        let merged = merge_configs([&first, &second]);

        assert_eq!(value_names(&merged.proxies), vec!["香港 A"]);
        assert_eq!(
            group_members(&merged, "B 组"),
            vec!["香港 A".to_string()],
            "references to a deduped proxy must resolve to the surviving name"
        );
        assert_eq!(
            merged.rules,
            vec!["DOMAIN,b.example.com,香港 A", "MATCH,PROXY"]
        );
    }

    #[test]
    fn merge_configs_renames_provider_collisions_and_rewrites_references() {
        const FIRST: &str = r#"
proxies:
  - name: A-1
    type: ss
    server: a.example.com
    port: 443
    cipher: aes-128-gcm
    password: a
rule-providers:
  reject:
    type: http
    behavior: domain
    url: https://a.example.com/reject.yaml
proxy-providers:
  pool:
    type: http
    url: https://a.example.com/pool.yaml
    path: ./providers/pool.yaml
proxy-groups:
  - name: 自动
    type: select
    use: [pool]
rules:
  - RULE-SET,reject,DIRECT
  - MATCH,自动
"#;
        const SECOND: &str = r#"
proxies:
  - name: B-1
    type: ss
    server: b.example.com
    port: 443
    cipher: aes-128-gcm
    password: b
rule-providers:
  reject:
    type: http
    behavior: domain
    url: https://b.example.com/reject.yaml
proxy-providers:
  pool:
    type: http
    url: https://b.example.com/pool.yaml
    path: ./providers/pool.yaml
proxy-groups:
  - name: 自动
    type: select
    use: [pool]
rules:
  - RULE-SET,reject,DIRECT
  - MATCH,自动
"#;
        let first = config(FIRST);
        let second = config(SECOND);
        let merged = merge_configs([&first, &second]);

        let provider_url = |key: &str, name: &str| -> Option<String> {
            merged
                .settings
                .get(key)?
                .as_mapping()?
                .get(name)?
                .as_mapping()?
                .get("url")?
                .as_str()
                .map(str::to_string)
        };
        assert_eq!(
            provider_url("rule-providers", "reject").as_deref(),
            Some("https://a.example.com/reject.yaml")
        );
        assert_eq!(
            provider_url("rule-providers", "reject 2").as_deref(),
            Some("https://b.example.com/reject.yaml")
        );
        assert_eq!(
            provider_url("proxy-providers", "pool").as_deref(),
            Some("https://a.example.com/pool.yaml")
        );
        assert_eq!(
            provider_url("proxy-providers", "pool 2").as_deref(),
            Some("https://b.example.com/pool.yaml")
        );
        assert_eq!(
            merged.rules,
            vec![
                "RULE-SET,reject,DIRECT",
                "RULE-SET,reject 2,DIRECT",
                "MATCH,PROXY"
            ],
            "colliding rule providers must be renamed and referenced accordingly"
        );
        assert_eq!(value_names(&merged.groups), vec!["PROXY", "自动", "自动 2"]);
        assert_eq!(group_uses(&merged, "自动"), vec!["pool".to_string()]);
        assert_eq!(
            group_uses(&merged, "自动 2"),
            vec!["pool 2".to_string()],
            "group use references must follow renamed proxy providers"
        );
    }

    #[test]
    fn merge_configs_dedupes_rules_across_sources() {
        const SOURCE: &str = r#"
proxies:
  - name: A-1
    type: ss
    server: a.example.com
    port: 443
    cipher: aes-128-gcm
    password: a
rules:
  - DOMAIN,dup.example.com,DIRECT
"#;
        let first = config(SOURCE);
        let second = config(SOURCE);
        let merged = merge_configs([&first, &second]);

        assert_eq!(
            merged.rules,
            vec!["DOMAIN,dup.example.com,DIRECT", "MATCH,PROXY"]
        );
    }

    #[test]
    fn merged_proxies_round_trip_unknown_fields() {
        let source = config(SOURCE_B);
        let merged = merge_configs([&source]);
        let hy2 = merged
            .proxies
            .iter()
            .find(|value| {
                value
                    .as_mapping()
                    .and_then(|mapping| mapping.get("type"))
                    .and_then(Value::as_str)
                    == Some("hysteria2")
            })
            .unwrap();
        let mapping = hy2.as_mapping().unwrap();
        assert_eq!(mapping.get("obfs").unwrap().as_str(), Some("salamander"));
        assert_eq!(
            mapping.get("obfs-password").unwrap().as_str(),
            Some("obfs-secret")
        );
    }
}
