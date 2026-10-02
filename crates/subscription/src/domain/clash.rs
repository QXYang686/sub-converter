use std::collections::HashSet;

use serde_yaml::{Mapping, Value};
use thiserror::Error;

pub const PROXY_GROUP_NAME: &str = "PROXY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProxyProtocol {
    Vmess,
    AnyTls,
    Hysteria2,
}

impl ProxyProtocol {
    pub fn from_type(raw: &str) -> Option<Self> {
        match raw {
            "vmess" => Some(Self::Vmess),
            "anytls" => Some(Self::AnyTls),
            "hysteria2" => Some(Self::Hysteria2),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vmess => "vmess",
            Self::AnyTls => "anytls",
            Self::Hysteria2 => "hysteria2",
        }
    }

    fn credential_key(&self) -> &'static str {
        match self {
            Self::Vmess => "uuid",
            Self::AnyTls | Self::Hysteria2 => "password",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClashProxy {
    protocol: ProxyProtocol,
    identity: String,
    value: Value,
}

impl ClashProxy {
    fn from_value(value: &Value) -> Option<Self> {
        let mapping = value.as_mapping()?;
        let protocol = mapping
            .get("type")?
            .as_str()
            .and_then(ProxyProtocol::from_type)?;
        let server = mapping
            .get("server")?
            .as_str()
            .map(str::trim)
            .filter(|server| !server.is_empty())?;
        let port = match mapping.get("port")? {
            Value::Number(number) => number.as_u64().filter(|port| (1..=65535).contains(port))?,
            _ => return None,
        };
        let credential = mapping
            .get(protocol.credential_key())?
            .as_str()
            .filter(|credential| !credential.is_empty())?;
        let name = mapping
            .get("name")?
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())?;
        let _ = name;

        let identity = format!("{}|{}|{}|{}", protocol.as_str(), server, port, credential);
        Some(Self {
            protocol,
            identity,
            value: value.clone(),
        })
    }

    pub fn protocol(&self) -> ProxyProtocol {
        self.protocol
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

pub fn parse(body: &str) -> Result<ParsedClash, ClashError> {
    let document: Value = serde_yaml::from_str(body).map_err(|_| ClashError::InvalidYaml)?;
    let mapping = document.as_mapping().ok_or(ClashError::InvalidYaml)?;
    let entries = mapping
        .get("proxies")
        .and_then(Value::as_sequence)
        .ok_or(ClashError::MissingProxies)?;

    let mut parsed = ParsedClash::default();
    for entry in entries {
        match ClashProxy::from_value(entry) {
            Some(proxy) => parsed.proxies.push(proxy),
            None => parsed.skipped += 1,
        }
    }
    Ok(parsed)
}

pub fn merge<'a, I>(sources: I) -> Vec<Value>
where
    I: IntoIterator<Item = &'a ParsedClash>,
{
    let mut identities: HashSet<String> = HashSet::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut merged = Vec::new();

    for parsed in sources {
        for proxy in &parsed.proxies {
            if !identities.insert(proxy.identity.clone()) {
                continue;
            }
            let mut value = proxy.value.clone();
            uniquify_name(&mut value, &mut names);
            merged.push(value);
        }
    }
    merged
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
    if used.insert(original.clone()) {
        return;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{original} {suffix}");
        if used.insert(candidate.clone()) {
            mapping.insert(Value::String("name".to_string()), Value::String(candidate));
            return;
        }
        suffix += 1;
    }
}

pub fn render(proxies: &[Value]) -> String {
    let mut group_proxies: Vec<Value> = proxies
        .iter()
        .filter_map(|proxy| proxy.as_mapping()?.get("name").cloned())
        .collect();
    group_proxies.push(Value::String("DIRECT".to_string()));

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
        Value::Sequence(group_proxies),
    );

    let mut root = Mapping::new();
    root.insert(
        Value::String("mode".to_string()),
        Value::String("rule".to_string()),
    );
    root.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxies.to_vec()),
    );
    root.insert(
        Value::String("proxy-groups".to_string()),
        Value::Sequence(vec![Value::Mapping(group)]),
    );
    root.insert(
        Value::String("rules".to_string()),
        Value::Sequence(vec![Value::String(format!("MATCH,{PROXY_GROUP_NAME}"))]),
    );

    serde_yaml::to_string(&Value::Mapping(root)).unwrap_or_default()
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
  - name: 不支持的协议
    type: trojan
    server: trojan.example.com
    port: 443
    password: trojan-secret
  - name: 畸形节点
    type: vmess
    server: broken.example.com
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
    cipher: auto
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
    fn parse_keeps_supported_protocols_and_skips_rest() {
        let result = parsed(SOURCE_A);
        assert_eq!(result.proxies().len(), 2);
        assert_eq!(result.skipped(), 2);
        assert_eq!(result.proxies()[0].protocol(), ProxyProtocol::Vmess);
        assert_eq!(result.proxies()[1].protocol(), ProxyProtocol::AnyTls);
        assert_eq!(result.proxies()[0].name(), Some("香港 01"));
    }

    #[test]
    fn parse_rejects_invalid_or_proxyless_documents() {
        assert_eq!(parse("not: [valid"), Err(ClashError::InvalidYaml));
        assert_eq!(parse("mode: rule"), Err(ClashError::MissingProxies));
        assert_eq!(parse("- just\n- a\n- list"), Err(ClashError::InvalidYaml));
    }

    #[test]
    fn merge_dedupes_identity_and_renames_conflicts() {
        let source_a = parsed(SOURCE_A);
        let source_b = parsed(SOURCE_B);
        let merged = merge([&source_a, &source_b]);
        let names: Vec<&str> = merged
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
            .collect();

        assert_eq!(
            names,
            vec!["香港 01", "日本 01", "香港 01 2", "新加坡 01"],
            "duplicate vmess across sources must be dropped, name conflict suffixed"
        );
    }

    #[test]
    fn merge_preserves_source_order() {
        let source_b = parsed(SOURCE_B);
        let source_a = parsed(SOURCE_A);
        let merged = merge([&source_b, &source_a]);
        let first = merged[0].as_mapping().unwrap().get("name").unwrap();
        assert_eq!(first.as_str(), Some("香港 01"));
        assert_eq!(merged.len(), 4);
    }

    #[test]
    fn render_produces_minimal_clash_config() {
        let source = parsed(SOURCE_A);
        let merged = merge([&source]);
        let output = render(&merged);
        let round_tripped: Value = serde_yaml::from_str(&output).unwrap();
        let root = round_tripped.as_mapping().unwrap();

        assert_eq!(root.get("mode").unwrap().as_str(), Some("rule"));
        assert_eq!(root.get("proxies").unwrap().as_sequence().unwrap().len(), 2);
        let group = root.get("proxy-groups").unwrap().as_sequence().unwrap()[0]
            .as_mapping()
            .unwrap()
            .clone();
        let group_proxies: Vec<&str> = group
            .get("proxies")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(group_proxies, vec!["香港 01", "日本 01", "DIRECT"]);
        let rules: Vec<&str> = root
            .get("rules")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(rules, vec!["MATCH,PROXY"]);
    }

    #[test]
    fn render_empty_config_falls_back_to_direct() {
        let output = render(&[]);
        let round_tripped: Value = serde_yaml::from_str(&output).unwrap();
        let root = round_tripped.as_mapping().unwrap();
        assert!(root
            .get("proxies")
            .unwrap()
            .as_sequence()
            .unwrap()
            .is_empty());
        let group = root.get("proxy-groups").unwrap().as_sequence().unwrap()[0]
            .as_mapping()
            .unwrap()
            .clone();
        let group_proxies: Vec<&str> = group
            .get("proxies")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        assert_eq!(group_proxies, vec!["DIRECT"]);
    }

    #[test]
    fn supported_proxy_round_trips_unknown_fields() {
        let result = parsed(SOURCE_B);
        let merged = merge([&result]);
        let hy2 = merged
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
