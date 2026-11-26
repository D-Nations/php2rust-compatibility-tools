//! Replays `tests/data/objects.json` (`scripts/record.sh date`,
//! `oracle/recorders/php_date_objects.php`): the date classes as
//! php2rust-value objects, their properties, methods, comparisons and
//! argument errors.
//!
//! Upstream: none
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::sync::Arc;

use php2rust_date::{
    DateTimeImmutable, DateTimeZone,
    object::{DateTime, date_of},
};
use php2rust_value::{PhpArray, PhpObject, Thrown, Value, compare::loose_equals, ops};
use serde_json::{Value as Json, json};

fn data() -> Json {
    serde_json::from_str(include_str!("data/objects.json")).expect("tests/data/objects.json")
}

fn array_json(array: &PhpArray) -> Json {
    let mut map = serde_json::Map::new();
    for (key, value) in array.iter() {
        let v = match value {
            Value::Int(i) => json!(i),
            Value::String(s) => json!(String::from_utf8_lossy(s.as_bytes())),
            other => panic!("unexpected property {other:?}"),
        };
        map.insert(String::from_utf8_lossy(&key.bytes()).into_owned(), v);
    }
    Json::Object(map)
}

fn call(object: &Value, name: &str, arguments: &[Value]) -> Result<Value, Thrown> {
    let Value::Object(o) = object else {
        panic!("not an object")
    };
    o.call_method(name, arguments).expect(name)
}

fn text(value: &Value) -> String {
    String::from_utf8_lossy(value.as_string().expect("a string").as_bytes()).into_owned()
}

fn thrown(e: &Thrown) -> Json {
    json!({"class": e.class, "message": String::from_utf8_lossy(e.message.as_bytes())})
}

fn make(case: &Json) -> Value {
    let zone = DateTimeZone::new(case["zone"].as_str().unwrap()).unwrap();
    let date = DateTimeImmutable::new(case["text"].as_str().unwrap(), Some(&zone)).unwrap();
    match case["class"].as_str().unwrap() {
        "DateTime" => DateTime::object(date),
        _ => Value::Object(Arc::new(date)),
    }
}

#[test]
fn dates() {
    let d = data();
    for case in d["dates"].as_array().unwrap() {
        let date = make(case);
        let Value::Object(o) = &date else {
            unreachable!()
        };
        assert_eq!(o.class(), case["class"], "{case}");
        assert_eq!(array_json(&o.properties()), case["array"], "{case}");
        assert_eq!(json!(o.instance_of("DateTimeInterface")), case["interface"]);
        let format = call(&date, "format", &["c u".into()]).unwrap();
        assert_eq!(json!(text(&format)), case["format"], "{case}");
        for (method, key) in [
            ("getTimestamp", "timestamp"),
            ("getMicrosecond", "microsecond"),
            ("getOffset", "offset"),
        ] {
            let Value::Int(i) = call(&date, method, &[]).unwrap() else {
                panic!("{method}")
            };
            assert_eq!(json!(i), case[key], "{method} {case}");
        }
        let zone = call(&date, "getTimezone", &[]).unwrap();
        assert_eq!(
            json!(text(&call(&zone, "getName", &[]).unwrap())),
            case["timezone"]
        );
    }
}

#[test]
fn compare() {
    let d = data();
    let dates: Vec<Value> = d["dates"].as_array().unwrap().iter().map(make).collect();
    for case in d["compare"].as_array().unwrap() {
        let a = &dates[usize::try_from(case["a"].as_u64().unwrap()).unwrap()];
        let b = &dates[usize::try_from(case["b"].as_u64().unwrap()).unwrap()];
        let order = ops::compare(a, b);
        assert_eq!(json!(loose_equals(a, b)), case["eq"], "{case}");
        assert_eq!(json!(order.is_lt()), case["lt"], "{case}");
        assert_eq!(json!(ops::compare(b, a).is_lt()), case["gt"], "{case}");
        assert_eq!(json!(ops::spaceship(a, b)), case["cmp"], "{case}");
    }
}

#[test]
fn zones() {
    let d = data();
    for case in d["zones"].as_array().unwrap() {
        let zone = DateTimeZone::new(case["name"].as_str().unwrap()).unwrap();
        assert_eq!(array_json(&zone.properties()), case["array"], "{case}");
        let object = Value::Object(Arc::new(zone));
        assert_eq!(
            json!(text(&call(&object, "getName", &[]).unwrap())),
            case["get_name"]
        );
    }
}

#[test]
fn calls() {
    let d = data();
    let paris = DateTimeZone::new("Europe/Paris").unwrap();
    let base = Value::Object(Arc::new(
        DateTimeImmutable::new("2010-10-04 13:45", Some(&paris)).unwrap(),
    ));
    for case in d["calls"].as_array().unwrap() {
        let arg = case["arg"].as_str().unwrap();
        let result = match case["call"].as_str().unwrap() {
            "setTimezone" => call(
                &base,
                "setTimezone",
                &[Value::Object(Arc::new(DateTimeZone::new(arg).unwrap()))],
            ),
            _ => call(&base, "modify", &[arg.into()]),
        };
        let actual = match result {
            Ok(date) => json!(date_of(&date).unwrap().format("c e")),
            Err(e) => thrown(&e),
        };
        assert_eq!(actual, case["result"], "{case}");
    }
    // modify() leaves the immutable base as it was.
    assert_eq!(
        text(&call(&base, "format", &["c".into()]).unwrap()),
        "2010-10-04T13:45:00+02:00"
    );

    let errors = &d["errors"];
    let array = Value::Array(PhpArray::new());
    let datetime = DateTime::object(DateTimeImmutable::new("2010-10-04 13:45", None).unwrap());
    for (key, result) in [
        (
            "format_array",
            call(&base, "format", std::slice::from_ref(&array)),
        ),
        (
            "modify_array",
            call(&base, "modify", std::slice::from_ref(&array)),
        ),
        (
            "set_timezone_string",
            call(&base, "setTimezone", &["UTC".into()]),
        ),
        (
            "datetime_format_array",
            call(&datetime, "format", std::slice::from_ref(&array)),
        ),
    ] {
        assert_eq!(thrown(&result.unwrap_err()), errors[key], "{key}");
    }
    // PHP's "" with a deprecation: refused.
    assert_eq!(errors["format_null"], json!(""));
    assert_eq!(
        call(&base, "format", &[Value::Null]).unwrap_err().class,
        "Unsupported"
    );
}

#[test]
fn mutable_datetime() {
    let date = DateTime::new(DateTimeImmutable::new("2010-10-04 13:45", None).unwrap());
    let copy = date.deep_clone();
    date.modify("-1day").unwrap();
    date.set_timezone(&DateTimeZone::new("Asia/Hong_Kong").unwrap());
    assert_eq!(
        date.get().format("Y-m-d H:i e"),
        "2010-10-03 21:45 Asia/Hong_Kong"
    );
    assert_eq!(copy.get().format("Y-m-d H:i e"), "2010-10-04 13:45 UTC");
    assert!(date.modify("Bad Date").is_err());
    assert_eq!(date.get().format("Y-m-d H:i"), "2010-10-03 21:45");
}
