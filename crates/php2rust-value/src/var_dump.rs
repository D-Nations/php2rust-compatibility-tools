//! `var_dump()` of scalars and arrays, as text (what PHP prints, without
//! output buffering).
//!
//! Written from PHP 8.4.27's observed output (`tests/data/var_dump.json`,
//! `oracle/recorders/php_value_var_dump.php`, `scripts/record.sh
//! value-var-dump`). No PHP source code is used. Objects are refused
//! ([`Unsupported`]): their output shows object handles (`#3`) and
//! property visibility, which a [`Value`] does not carry.
//!
//! Upstream: none

use std::fmt;

use crate::array::Key;
use crate::numeric::var_dump_float;
use crate::value::Value;

/// A value `var_dump()` cannot print here: an object, with its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    /// The object's class.
    pub class: String,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "var_dump() of an object of class {} is not supported",
            self.class
        )
    }
}

impl std::error::Error for Unsupported {}

/// `var_dump(...$values)`: each value in turn.
pub fn var_dump(values: &[Value]) -> Result<Vec<u8>, Unsupported> {
    let mut out = Vec::new();
    for value in values {
        dump(value, 0, &mut out)?;
    }
    Ok(out)
}

fn indent(out: &mut Vec<u8>, depth: usize) {
    out.resize(out.len() + depth, b' ');
}

fn dump(value: &Value, depth: usize, out: &mut Vec<u8>) -> Result<(), Unsupported> {
    indent(out, depth);
    match value {
        Value::Null => out.extend_from_slice(b"NULL\n"),
        Value::Bool(true) => out.extend_from_slice(b"bool(true)\n"),
        Value::Bool(false) => out.extend_from_slice(b"bool(false)\n"),
        Value::Int(i) => out.extend_from_slice(format!("int({i})\n").as_bytes()),
        Value::Float(f) => {
            out.extend_from_slice(format!("float({})\n", var_dump_float(*f)).as_bytes());
        }
        Value::String(s) => {
            out.extend_from_slice(format!("string({}) \"", s.len()).as_bytes());
            out.extend_from_slice(s.as_bytes());
            out.extend_from_slice(b"\"\n");
        }
        Value::Array(array) => {
            out.extend_from_slice(format!("array({}) {{\n", array.len()).as_bytes());
            for (key, item) in array.iter() {
                indent(out, depth + 2);
                match key {
                    Key::Int(i) => out.extend_from_slice(format!("[{i}]=>\n").as_bytes()),
                    Key::Str(s) => {
                        out.extend_from_slice(b"[\"");
                        out.extend_from_slice(s.as_bytes());
                        out.extend_from_slice(b"\"]=>\n");
                    }
                }
                dump(item, depth + 2, out)?;
            }
            indent(out, depth);
            out.extend_from_slice(b"}\n");
        }
        Value::Object(object) => {
            return Err(Unsupported {
                class: object.class().to_owned(),
            });
        }
    }
    Ok(())
}
