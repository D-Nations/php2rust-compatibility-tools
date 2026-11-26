//! `var_dump()` against PHP 8.4.27's output recorded by
//! `oracle/recorders/php_value_var_dump.php` (`scripts/record.sh
//! value-var-dump`): scalars, floats, byte strings and nested arrays, one
//! or more arguments.
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use php2rust_value::{
    Value,
    object::StdClass,
    var_dump::{Unsupported, var_dump},
    vectors,
};
use serde_json::Value as Json;

#[test]
fn var_dump_matches_php() {
    let data: Json = serde_json::from_str(include_str!("data/var_dump.json")).unwrap();
    let cases = data["cases"].as_array().unwrap();
    assert!(cases.len() > 40);
    for case in cases {
        let args: Vec<Value> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| vectors::decode(a).unwrap())
            .collect();
        let expected = vectors::base64(case["out"].as_str().unwrap()).unwrap();
        let got = var_dump(&args).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&expected),
            "var_dump({args:?})"
        );
        assert_eq!(got, expected);
    }
}

#[test]
fn objects_are_refused() {
    let object = Value::Object(Arc::new(StdClass::default()));
    assert_eq!(
        var_dump(&[Value::Array(php2rust_value::PhpArray::from_iter([(
            php2rust_value::Key::Int(0),
            object
        )]))]),
        Err(Unsupported {
            class: "stdClass".to_owned()
        })
    );
}
