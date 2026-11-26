//! `args::bind()` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/php_value_named_args.php` (`scripts/record.sh
//! value-args`): `$object->m(...$arguments)` with positional and named
//! arguments for eight signatures.
//!
//! Upstream: none

use php2rust_value::{PhpArray, Value, args, vectors};
use serde_json::Value as Json;

#[test]
fn bind_matches_php() -> Result<(), vectors::Error> {
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/named_args.json"))?;
    assert!(cases.len() >= 800, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let mut parameters = Vec::new();
        for p in case[1].as_array().ok_or("parameters")? {
            let name = p[0].as_str().ok_or("name")?;
            parameters.push(if p[1].is_null() {
                args::Parameter::required(name)
            } else {
                args::Parameter::optional(name, vectors::decode(&p[1])?)
            });
        }
        let arguments = Json::Array(vec![Json::from("array"), case[2].clone()]);
        let Value::Array(arguments) = vectors::decode(&arguments)? else {
            return Err("arguments".into());
        };
        let function = format!("Signature{}::m", case[0]);
        let expected = &case[3];
        let ok = match (
            args::bind(&function, &parameters, &arguments),
            expected[0].as_str(),
        ) {
            (Ok(values), Some("array")) => {
                let mut list = PhpArray::new();
                for v in values {
                    list.push(v).map_err(|_| "push")?;
                }
                Value::Array(list) == vectors::decode(expected)?
            }
            (Err(e), Some("throws")) => {
                e.class == expected[1].as_str().unwrap_or_default()
                    && e.message.as_bytes() == expected[2].as_str().unwrap_or_default().as_bytes()
            }
            _ => false,
        };
        if !ok {
            let got = args::bind(&function, &parameters, &arguments);
            failures.push(format!(
                "{function} {}: PHP {expected}, Rust {got:?}",
                case[2]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    Ok(())
}
