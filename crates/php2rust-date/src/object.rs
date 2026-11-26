//! The date classes as php2rust-value objects (`Value::Object`):
//! `DateTimeImmutable`, a mutable `DateTime` handle and `DateTimeZone`.
//! Each has its class, `instanceof` (`DateTimeInterface`), the properties
//! `var_dump()` and `(array)` show, and PHP's comparison by instant (`==`,
//! `<`, `<=>` between any two dates, whatever their class or zone).
//!
//! Methods through [`PhpObject::call_method`]: `format()`,
//! `getTimestamp()`, `getMicrosecond()`, `getOffset()`, `getTimezone()`
//! on both date classes, and `setTimezone()` and `modify()` on
//! `DateTimeImmutable` (they return a new object). The `DateTime` methods
//! that change the object and return `$this` are Rust methods on
//! [`DateTime`] ([`DateTime::set_timezone`], [`DateTime::modify`]): a
//! method call here has no handle to return. `DateTimeZone` has
//! `getName()`.
//!
//! Upstream: none

use std::{
    cmp::Ordering,
    sync::{Arc, Mutex, PoisonError},
};

use php2rust_value::{PhpArray, PhpObject, PhpString, Thrown, Value};

use crate::{DateError, DateTimeImmutable, DateTimeZone};

/// A [`DateError`] as the exception PHP throws.
pub fn thrown(error: &DateError) -> Thrown {
    Thrown::new(error.class(), error.message().as_bytes().to_vec(), 0)
}

/// The date a value holds, for a `DateTimeImmutable` or `DateTime` object.
pub fn date_of(value: &Value) -> Option<DateTimeImmutable> {
    match value {
        Value::Object(o) => date_of_object(&**o),
        _ => None,
    }
}

fn date_of_object(object: &dyn PhpObject) -> Option<DateTimeImmutable> {
    object
        .downcast_ref::<DateTimeImmutable>()
        .cloned()
        .or_else(|| object.downcast_ref::<DateTime>().map(DateTime::get))
}

/// The zone a value holds, for a `DateTimeZone` object.
pub fn zone_of(value: &Value) -> Option<DateTimeZone> {
    match value {
        Value::Object(o) => o.downcast_ref::<DateTimeZone>().cloned(),
        _ => None,
    }
}

/// What `var_dump()` and `(array)` show for a date: `date`,
/// `timezone_type`, `timezone`.
fn date_properties(date: &DateTimeImmutable) -> PhpArray {
    let mut properties = PhpArray::new();
    properties.insert("date", date.format("Y-m-d H:i:s.u").into());
    let zone = date.get_timezone();
    properties.insert("timezone_type", i64::from(zone.timezone_type()).into());
    properties.insert("timezone", zone.get_name().into());
    properties
}

fn type_error(method: &str, position: usize, expected: &str, given: &Value) -> Thrown {
    Thrown::new(
        "TypeError",
        format!(
            "{method}(): Argument #{position} (${}) must be of type {expected}, {} given",
            match (method, position) {
                (m, 1) if m.ends_with("setTimezone") => "timezone",
                (m, 1) if m.ends_with("modify") => "modifier",
                _ => "format",
            },
            given.debug_type()
        )
        .into_bytes(),
        0,
    )
}

/// The string argument `position` (from 1) of `method`.
fn string_argument(method: &str, arguments: &[Value], position: usize) -> Result<String, Thrown> {
    let value = arguments.get(position - 1).cloned().unwrap_or(Value::Null);
    match &value {
        Value::String(s) => Ok(String::from_utf8_lossy(s.as_bytes()).into_owned()),
        // PHP reads null as "" with a deprecation, which this crate does
        // not raise: refused.
        Value::Null => Err(Thrown::new(
            "Unsupported",
            format!("{method}(): a null argument is not supported by this port").into_bytes(),
            0,
        )),
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => value
            .to_php_string()
            .map(|s| String::from_utf8_lossy(s.as_bytes()).into_owned())
            .ok_or_else(|| type_error(method, position, "string", &value)),
        _ => Err(type_error(method, position, "string", &value)),
    }
}

/// The methods both date classes answer the same way.
fn read_method(
    class: &str,
    date: &DateTimeImmutable,
    name: &str,
    arguments: &[Value],
) -> Option<Result<Value, Thrown>> {
    Some(match name.to_ascii_lowercase().as_str() {
        "format" => string_argument(&format!("{class}::format"), arguments, 1)
            .map(|format| Value::String(PhpString::from(date.format(&format)))),
        "gettimestamp" => Ok(Value::Int(date.get_timestamp())),
        "getmicrosecond" => Ok(Value::Int(i64::from(date.get_microsecond()))),
        "getoffset" => Ok(Value::Int(i64::from(date.get_offset()))),
        "gettimezone" => Ok(Value::Object(Arc::new(date.get_timezone()))),
        _ => return None,
    })
}

const READ_METHODS: [&str; 5] = [
    "format",
    "getTimestamp",
    "getMicrosecond",
    "getOffset",
    "getTimezone",
];

impl PhpObject for DateTimeImmutable {
    fn class(&self) -> &'static str {
        "DateTimeImmutable"
    }

    fn properties(&self) -> PhpArray {
        date_properties(self)
    }

    fn instance_of(&self, class: &str) -> bool {
        ["DateTimeImmutable", "DateTimeInterface"]
            .iter()
            .any(|c| c.eq_ignore_ascii_case(class.trim_start_matches('\\')))
    }

    fn methods(&self) -> Vec<String> {
        READ_METHODS
            .iter()
            .chain(&["setTimezone", "modify"])
            .map(|m| (*m).to_owned())
            .collect()
    }

    fn call_method(&self, name: &str, arguments: &[Value]) -> Option<Result<Value, Thrown>> {
        if let Some(result) = read_method("DateTimeImmutable", self, name, arguments) {
            return Some(result);
        }
        Some(match name.to_ascii_lowercase().as_str() {
            "settimezone" => {
                let argument = arguments.first().cloned().unwrap_or(Value::Null);
                match zone_of(&argument) {
                    Some(zone) => Ok(Value::Object(Arc::new(self.set_timezone(&zone)))),
                    None => Err(type_error(
                        "DateTimeImmutable::setTimezone",
                        1,
                        "DateTimeZone",
                        &argument,
                    )),
                }
            }
            "modify" => {
                string_argument("DateTimeImmutable::modify", arguments, 1).and_then(|modifier| {
                    self.modify(&modifier)
                        .map(|date| Value::Object(Arc::new(date)))
                        .map_err(|e| thrown(&e))
                })
            }
            _ => return None,
        })
    }

    fn compare_to(&self, other: &dyn PhpObject) -> Option<Ordering> {
        let other = date_of_object(other)?;
        self.partial_cmp(&other)
    }
}

/// A mutable `DateTime`: one object whose instant and zone
/// [`DateTime::set_timezone`] and [`DateTime::modify`] change in place
/// (`clone` is [`DateTime::deep_clone`]).
#[derive(Debug)]
pub struct DateTime(Mutex<DateTimeImmutable>);

impl DateTime {
    pub fn new(date: DateTimeImmutable) -> Self {
        Self(Mutex::new(date))
    }

    /// A new `DateTime` object holding `date`.
    pub fn object(date: DateTimeImmutable) -> Value {
        Value::Object(Arc::new(Self::new(date)))
    }

    /// The current instant and zone.
    pub fn get(&self) -> DateTimeImmutable {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn replace(&self, date: DateTimeImmutable) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = date;
    }

    /// `$date->setTimezone($zone)`, in place.
    pub fn set_timezone(&self, zone: &DateTimeZone) {
        self.replace(self.get().set_timezone(zone));
    }

    /// `$date->modify($modifier)`, in place. On an error the date is
    /// unchanged.
    pub fn modify(&self, modifier: &str) -> Result<(), DateError> {
        let date = self.get().modify(modifier)?;
        self.replace(date);
        Ok(())
    }

    /// `clone $date`: a new object with the same instant and zone.
    #[must_use]
    pub fn deep_clone(&self) -> Self {
        Self::new(self.get())
    }
}

impl PhpObject for DateTime {
    fn class(&self) -> &'static str {
        "DateTime"
    }

    fn properties(&self) -> PhpArray {
        date_properties(&self.get())
    }

    fn instance_of(&self, class: &str) -> bool {
        ["DateTime", "DateTimeInterface"]
            .iter()
            .any(|c| c.eq_ignore_ascii_case(class.trim_start_matches('\\')))
    }

    fn methods(&self) -> Vec<String> {
        READ_METHODS.iter().map(|m| (*m).to_owned()).collect()
    }

    fn call_method(&self, name: &str, arguments: &[Value]) -> Option<Result<Value, Thrown>> {
        read_method("DateTime", &self.get(), name, arguments)
    }

    fn compare_to(&self, other: &dyn PhpObject) -> Option<Ordering> {
        let other = date_of_object(other)?;
        self.get().partial_cmp(&other)
    }
}

impl PhpObject for DateTimeZone {
    fn class(&self) -> &'static str {
        "DateTimeZone"
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("timezone_type", i64::from(self.timezone_type()).into());
        properties.insert("timezone", self.get_name().into());
        properties
    }

    fn methods(&self) -> Vec<String> {
        vec!["getName".to_owned()]
    }

    fn call_method(&self, name: &str, _arguments: &[Value]) -> Option<Result<Value, Thrown>> {
        name.eq_ignore_ascii_case("getName")
            .then(|| Ok(self.get_name().into()))
    }
}
