use serde_json::{Map, Number, Value as JsonValue};
use serde_yaml::Value;

pub(crate) fn yaml_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(value) => JsonValue::Bool(*value),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                JsonValue::Number(Number::from(value))
            } else if let Some(value) = value.as_f64() {
                Number::from_f64(value)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            } else {
                JsonValue::Null
            }
        }
        Value::String(value) => JsonValue::String(value.clone()),
        Value::Sequence(sequence) => JsonValue::Array(sequence.iter().map(yaml_to_json).collect()),
        Value::Mapping(mapping) => {
            let mut object = Map::new();
            for (key, value) in mapping {
                let key = match key {
                    Value::String(key) => key.clone(),
                    Value::Number(number) => number.to_string(),
                    Value::Bool(value) => value.to_string(),
                    _ => continue,
                };
                object.insert(key, yaml_to_json(value));
            }
            JsonValue::Object(object)
        }
        Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

pub(crate) fn json_to_yaml(value: &JsonValue) -> Value {
    match value {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(value) => Value::Bool(*value),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                Value::Number(value.into())
            } else if let Some(value) = value.as_u64() {
                Value::Number(value.into())
            } else {
                value
                    .as_f64()
                    .map(|value| Value::Number(value.into()))
                    .unwrap_or(Value::Null)
            }
        }
        JsonValue::String(value) => Value::String(value.clone()),
        JsonValue::Array(sequence) => Value::Sequence(sequence.iter().map(json_to_yaml).collect()),
        JsonValue::Object(object) => Value::Mapping(
            object
                .iter()
                .map(|(key, value)| (Value::String(key.clone()), json_to_yaml(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_nested_yaml_to_json() {
        let yaml: Value = serde_yaml::from_str(
            "port: 443\ntls: true\nname: 香港\nalpn: [h2, http/1.1]\nws-opts:\n  path: /ws\n",
        )
        .unwrap();
        let json = yaml_to_json(&yaml);
        assert_eq!(json["port"], 443);
        assert_eq!(json["tls"], true);
        assert_eq!(json["name"], "香港");
        assert_eq!(json["alpn"][0], "h2");
        assert_eq!(json["ws-opts"]["path"], "/ws");
    }

    #[test]
    fn yaml_to_json_round_trips_through_json_to_yaml() {
        let yaml: Value = serde_yaml::from_str(
            "name: 香港 01\nport: 443\ntls: true\nalpn: [h2, http/1.1]\nws-opts:\n  path: /ws\n",
        )
        .unwrap();
        let round_tripped = json_to_yaml(&yaml_to_json(&yaml));
        assert_eq!(round_tripped, yaml);
    }
}
