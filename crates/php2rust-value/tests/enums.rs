//! Enum cases against PHP 8.4.27's answers recorded by
//! `oracle/recorders/php_value_enums.php` (`scripts/record.sh
//! value-enums`): `instanceof`, `(array)`, `enum_exists()` and the static
//! methods `cases()`, `from()` and `tryFrom()` called on a case.
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::expect_used)]

use php2rust_value::{PhpObject, Value, constants::Registry, object::EnumCase};
use serde_json::{Value as Json, json};

fn registry() -> Registry {
    let mut registry = Registry::empty();
    registry.define_enum(
        "Suit",
        vec![
            EnumCase::new("Suit", "Hearts", None),
            EnumCase::new("Suit", "Spades", None),
        ],
    );
    registry.define_enum(
        "Status",
        vec![
            EnumCase::new("Status", "Active", Some("active".into())),
            EnumCase::new("Status", "Inactive", Some("inactive".into())),
        ],
    );
    registry.define_enum(
        "Level",
        vec![
            EnumCase::new("Level", "Low", Some(Value::Int(1))),
            EnumCase::new("Level", "High", Some(Value::Int(10))),
        ],
    );
    registry.define_enum("Nothing", Vec::new());
    registry
}

fn show(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Int(i) => json!(i),
        Value::String(s) => json!(String::from_utf8_lossy(s.as_bytes())),
        Value::Object(o) => {
            let case = o.downcast_ref::<EnumCase>().expect("an enum case");
            json!({"case": format!("{}::{}", o.class(), case.name())})
        }
        Value::Array(a) => Json::Array(a.values().map(show).collect()),
        other => panic!("unexpected {other:?}"),
    }
}

fn argument(json: &Json) -> Value {
    match json {
        Json::String(s) => s.as_str().into(),
        Json::Number(n) => Value::Int(n.as_i64().unwrap()),
        other => panic!("unexpected {other}"),
    }
}

#[test]
fn cases_match_php() {
    let data: Json = serde_json::from_str(include_str!("data/enums.json")).unwrap();
    let registry = registry();
    for row in data["cases"].as_array().unwrap() {
        let name = row["case"].as_str().unwrap();
        let Some(Value::Object(case)) = registry.constant(name).cloned() else {
            panic!("{name}")
        };
        assert_eq!(json!(case.instance_of("UnitEnum")), row["unit"], "{name}");
        assert_eq!(
            json!(case.instance_of("BackedEnum")),
            row["backed"],
            "{name}"
        );
        let array: serde_json::Map<String, Json> = case
            .properties()
            .iter()
            .map(|(k, v)| (String::from_utf8_lossy(&k.bytes()).into_owned(), show(v)))
            .collect();
        assert_eq!(Json::Object(array), row["array"], "{name}");
        let cases = case.call_method("cases", &[]).unwrap().unwrap();
        assert_eq!(show(&cases), row["cases"], "{name}");
        for call in row["calls"].as_array().unwrap() {
            let method = call["method"].as_str().unwrap();
            let actual = match case.call_method(method, &[argument(&call["arg"])]) {
                // PHP: "Call to undefined method"; a unit enum has no
                // from() here either.
                None => {
                    json!({"class": "Error", "message": format!("Call to undefined method {}::{method}()", case.class())})
                }
                Some(Ok(v)) => show(&v),
                Some(Err(e)) => {
                    json!({"class": e.class, "message": String::from_utf8_lossy(e.message.as_bytes())})
                }
            };
            assert_eq!(actual, call["result"], "{name}->{method}({})", call["arg"]);
        }
    }
    for (class, exists) in data["enum_exists"].as_object().unwrap() {
        assert_eq!(
            json!(registry.enum_exists(class)),
            *exists,
            "enum_exists({class})"
        );
    }
    assert_eq!(
        json!(registry.cases("Nothing").unwrap().len()),
        json!(data["empty_cases"].as_array().unwrap().len())
    );
}

#[test]
fn case_without_declaration_has_no_static_methods() {
    let case = EnumCase::new("Suit", "Hearts", None);
    assert!(case.call_method("cases", &[]).is_none());
    assert!(case.methods().is_empty());
}
