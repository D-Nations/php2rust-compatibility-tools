//! Replays `tests/data/php_date.json`, what PHP 8.4.27 answered in the
//! oracle (`scripts/record.sh date`, `oracle/recorders/php_date.php`): time
//! zones, date texts (`date_parse()`, the constructor, `modify()`),
//! `format()`, `createFromFormat()`, `createFromTimestamp()` and the
//! setters.
//!
//! Every answer this port gives must be PHP's. A text or format outside
//! what the port reads is refused (`DateError::Unsupported`); each
//! section pins how many recorded cases are refused, so a change in the
//! refused set shows up here.
//!
//! Upstream: none
#![allow(clippy::expect_used)]

use php2rust_date::{DateError, DateTimeImmutable, DateTimeZone, FormatError, date_parse};
use serde_json::{Value, json};

const F: &str = "Y-m-d H:i:s.u P e T I";

fn data() -> Value {
    let text = include_str!("data/php_date.json");
    serde_json::from_str(text).expect("tests/data/php_date.json")
}

fn thrown(e: &DateError) -> Value {
    json!({"class": e.class(), "message": e.message()})
}

/// Compares one answer; refusals are counted, not compared.
struct Check {
    section: &'static str,
    mismatches: Vec<String>,
    refused: Vec<String>,
}

impl Check {
    fn new(section: &'static str) -> Self {
        Self {
            section,
            mismatches: Vec::new(),
            refused: Vec::new(),
        }
    }

    fn compare(&mut self, context: &str, expected: &Value, actual: Result<Value, DateError>) {
        match actual {
            Err(DateError::Unsupported(_)) => self.refused.push(context.to_owned()),
            Err(e) => self.equal(context, expected, &thrown(&e)),
            Ok(v) => self.equal(context, expected, &v),
        }
    }

    fn equal(&mut self, context: &str, expected: &Value, actual: &Value) {
        if expected != actual {
            self.mismatches
                .push(format!("{context}: expected {expected}, got {actual}"));
        }
    }

    fn finish(self, refused: usize) {
        let shown: Vec<_> = self.mismatches.iter().take(40).collect();
        assert!(
            self.mismatches.is_empty(),
            "{}: {} mismatches:\n{}",
            self.section,
            self.mismatches.len(),
            shown
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(
            refused,
            self.refused.len(),
            "{}: refused cases changed: {:#?}",
            self.section,
            self.refused
        );
    }
}

fn zone(name: &str) -> DateTimeZone {
    DateTimeZone::new(name).expect(name)
}

/// The recorder's fixed dates: `new DateTimeImmutable($text, $zone ??
/// UTC)`, moved into `$zone` when the text names UTC.
fn base(text: &str, zone_name: Option<&str>) -> DateTimeImmutable {
    let z = zone(zone_name.unwrap_or("UTC"));
    let d = DateTimeImmutable::new(text, Some(&z)).expect(text);
    if zone_name.is_some() && text.ends_with("UTC") {
        d.set_timezone(&z)
    } else {
        d
    }
}

fn opt(v: Option<i64>) -> Value {
    v.map_or(Value::Bool(false), Value::from)
}

#[test]
fn default_timezone() {
    let d = data();
    assert_eq!(
        d["default_timezone"],
        json!(php2rust_date::date_default_timezone_get().get_name())
    );
}

#[test]
fn timezones() {
    let d = data();
    let mut check = Check::new("timezones");
    for case in d["timezones"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let actual = DateTimeZone::new(name)
            .map(|z| json!({"name": z.get_name(), "type": z.timezone_type()}));
        check.compare(name, &case["result"], actual);
    }
    check.finish(4);
}

#[test]
fn parse() {
    let d = data();
    let mut check = Check::new("parse");
    for case in d["parse"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let expected = &case["date_parse"];
        match date_parse(text) {
            Err(DateError::Unsupported(_)) => check.refused.push(format!("date_parse {text:?}")),
            Err(e) => {
                if expected["error_count"] == 0 && !text.trim().is_empty() {
                    check.mismatches.push(format!("date_parse {text:?}: {e}"));
                }
            }
            Ok(p) => {
                let actual = json!({
                    "year": opt(p.year), "month": opt(p.month), "day": opt(p.day),
                    "hour": opt(p.hour), "minute": opt(p.minute), "second": opt(p.second),
                    "fraction": opt(p.fraction), "error_count": 0,
                });
                check.equal(&format!("date_parse {text:?}"), expected, &actual);
            }
        }
        for (z, expected) in case["ctor"].as_object().unwrap() {
            let context = format!("new DateTimeImmutable({text:?}, {z})");
            let actual = DateTimeImmutable::new(text, Some(&zone(z))).map(|date| {
                let mut r = json!({
                    "zone": date.get_timezone().get_name(),
                    "type": date.get_timezone().timezone_type(),
                });
                if expected.get("date").is_some() {
                    r["date"] = json!(date.format(F));
                }
                if expected.get("time").is_some() {
                    r["time"] = json!(date.format("H:i:s.u"));
                }
                r
            });
            check.compare(&context, expected, actual);
        }
    }
    check.finish(275);
}

#[test]
fn modify() {
    let d = data();
    let bases: Vec<DateTimeImmutable> = d["bases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let date = base(b["text"].as_str().unwrap(), b["zone"].as_str());
            assert_eq!(b["date"], json!(date.format(F)), "base {b}");
            date
        })
        .collect();
    let mut check = Check::new("modify");
    for case in d["modify"].as_array().unwrap() {
        let i = usize::try_from(case["base"].as_u64().unwrap()).unwrap();
        let text = case["text"].as_str().unwrap();
        let actual = bases[i].modify(text).map(|r| json!(r.format(F)));
        check.compare(
            &format!("{} modify {text:?}", bases[i].format(F)),
            &case["result"],
            actual,
        );
    }
    check.finish(1100);
}

#[test]
fn format() {
    let d = data();
    let mut check = Check::new("format");
    for case in d["format"].as_array().unwrap() {
        let date = DateTimeImmutable::create_from_timestamp(case["timestamp"].as_i64().unwrap())
            .unwrap()
            .set_timezone(&zone(case["zone"].as_str().unwrap()))
            .set_microsecond(case["microsecond"].as_i64().unwrap())
            .unwrap();
        let actual = date.format(case["format"].as_str().unwrap());
        check.equal(&format!("{case}"), &case["result"], &json!(actual));
    }
    check.finish(0);
}

#[test]
fn from_format() {
    let d = data();
    let mut check = Check::new("from_format");
    for case in d["from_format"].as_array().unwrap() {
        let format = case["format"].as_str().unwrap();
        let string = case["string"].as_str().unwrap();
        let z = zone(case["zone"].as_str().unwrap());
        let context = format!("createFromFormat({format:?}, {string:?}, {})", z.get_name());
        match DateTimeImmutable::create_from_format(format, string, Some(&z)) {
            Ok(date) => {
                check.equal(&context, &case["result"], &json!(date.format(F)));
            }
            Err(FormatError::Failed(errors)) => {
                check.equal(&context, &case["result"], &json!(false));
                let expected = &case["last_errors"];
                let positions = |m: &std::collections::BTreeMap<usize, String>| {
                    m.iter()
                        .map(|(k, v)| (k.to_string(), json!(v)))
                        .collect::<serde_json::Map<_, _>>()
                };
                let mut actual_errors = positions(&errors.errors);
                let mut expected_errors = expected["errors"].clone();
                // PHP lists errors keyed 0, 1, ... as a JSON array.
                if let Some(list) = expected_errors.as_array() {
                    expected_errors = Value::Object(
                        list.iter()
                            .enumerate()
                            .map(|(i, v)| (i.to_string(), v.clone()))
                            .collect(),
                    );
                }
                if actual_errors.is_empty() {
                    actual_errors = serde_json::Map::new();
                }
                check.equal(
                    &format!("{context} errors"),
                    &expected_errors,
                    &Value::Object(actual_errors),
                );
                check.equal(
                    &format!("{context} error_count"),
                    &expected["error_count"],
                    &json!(errors.error_count),
                );
            }
            Err(FormatError::Date(_)) => check.refused.push(context.clone()),
        }
    }
    check.finish(0);
}

#[test]
fn timestamps() {
    let d = data();
    let mut check = Check::new("timestamps");
    for case in d["timestamps"].as_array().unwrap() {
        let value = &case["value"];
        let actual = if case["int"] == true {
            DateTimeImmutable::create_from_timestamp(value.as_i64().unwrap())
        } else {
            let x = match value.as_str() {
                Some("NAN") => f64::NAN,
                Some("INF") => f64::INFINITY,
                Some("-INF") => f64::NEG_INFINITY,
                _ => value.as_f64().unwrap(),
            };
            DateTimeImmutable::create_from_timestamp(x)
        };
        check.compare(
            &format!("createFromTimestamp({value})"),
            &case["result"],
            actual.map(|r| json!(r.format(F))),
        );
    }
    check.finish(0);
}

#[test]
fn setters() {
    let d = data();
    let bases: Vec<DateTimeImmutable> = d["setter_bases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let date = DateTimeImmutable::create_from_timestamp(b["timestamp"].as_i64().unwrap())
                .unwrap()
                .set_timezone(&zone(b["zone"].as_str().unwrap()))
                .set_microsecond(b["microsecond"].as_i64().unwrap())
                .unwrap();
            assert_eq!(b["date"], json!(date.format(F)), "setter base {b}");
            date
        })
        .collect();
    let mut check = Check::new("setters");
    for case in d["setters"].as_array().unwrap() {
        let i = usize::try_from(case["base"].as_u64().unwrap()).unwrap();
        let method = case["method"].as_str().unwrap();
        let args: Vec<i64> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_i64)
            .collect();
        let arg = |k: usize| args.get(k).copied().unwrap_or(0);
        let native = &bases[i];
        let n = match method {
            "setTime" => native.set_time(arg(0), arg(1), arg(2), arg(3)),
            "setDate" => native.set_date(arg(0), arg(1), arg(2)),
            "setISODate" => {
                let day = if args.len() > 2 { arg(2) } else { 1 };
                native.set_iso_date(arg(0), arg(1), day)
            }
            "setTimestamp" => Ok(native.set_timestamp(arg(0))),
            "setMicrosecond" => native.set_microsecond(arg(0)),
            "setTimezone" => Ok(native.set_timezone(&zone(case["args"][0].as_str().unwrap()))),
            other => panic!("unknown setter {other}"),
        };
        let context = format!("{} {method}{:?}", native.format(F), case["args"]);
        check.compare(&context, &case["native"], n.map(|r| json!(r.format(F))));
    }
    check.finish(5);
}
