use crate::array::PhpArray;
use crate::format;
use crate::numeric::{self, Number, Numericity};
use crate::string::PhpString;

/// A PHP value (the equivalent of a Zend `zval`).
///
/// Cloning is cheap: strings and arrays are reference-counted and copied only
/// when a shared copy is written to, which is how PHP's value semantics work.
#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(PhpString),
    Array(PhpArray),
}

impl Value {
    pub fn from_bytes(bytes: &[u8]) -> Value {
        Value::Str(PhpString::from(bytes))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// The type name PHP uses in error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
        }
    }

    /// The type name `gettype()` reports.
    pub fn gettype(&self) -> &'static str {
        match self {
            Value::Null => "NULL",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Float(_) => "double",
            Value::Str(_) => "string",
            Value::Array(_) => "array",
        }
    }

    /// `(bool)` conversion.
    pub fn to_bool(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !(s.is_empty() || s.as_bytes() == b"0"),
            Value::Array(a) => !a.is_empty(),
        }
    }

    /// `(int)` conversion. Never warns.
    pub fn to_int(&self) -> i64 {
        match self {
            Value::Null => 0,
            Value::Bool(b) => *b as i64,
            Value::Int(i) => *i,
            Value::Float(f) => numeric::float_to_int(*f),
            Value::Str(s) => match numeric::classify(s.as_bytes()) {
                Numericity::Numeric(Number::Int(i)) | Numericity::Leading(Number::Int(i)) => i,
                // Strings saturate rather than wrap.
                Numericity::Numeric(Number::Float(f)) | Numericity::Leading(Number::Float(f)) => {
                    if f.is_nan() { 0 } else { f as i64 }
                }
                Numericity::NonNumeric => 0,
            },
            Value::Array(a) => !a.is_empty() as i64,
        }
    }

    /// `(float)` conversion. Never warns.
    pub fn to_float(&self) -> f64 {
        match self {
            Value::Float(f) => *f,
            Value::Str(s) => match numeric::classify(s.as_bytes()) {
                Numericity::Numeric(n) | Numericity::Leading(n) => n.as_f64(),
                Numericity::NonNumeric => 0.0,
            },
            other => other.to_int() as f64,
        }
    }

    /// `(string)` conversion. Arrays become `"Array"`; use [`crate::rt::to_str`]
    /// where PHP would also emit the "Array to string conversion" warning.
    pub fn to_php_string(&self) -> PhpString {
        match self {
            Value::Str(s) => s.clone(),
            other => PhpString::from(other.to_string_bytes()),
        }
    }

    pub(crate) fn to_string_bytes(&self) -> Vec<u8> {
        match self {
            Value::Null | Value::Bool(false) => Vec::new(),
            Value::Bool(true) => b"1".to_vec(),
            Value::Int(i) => i.to_string().into_bytes(),
            Value::Float(f) => format::float_to_string(*f).into_bytes(),
            Value::Str(s) => s.as_bytes().to_vec(),
            Value::Array(_) => b"Array".to_vec(),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<usize> for Value {
    fn from(i: usize) -> Self {
        Value::Int(i as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(PhpString::from(s))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(PhpString::from(s))
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Str(PhpString::from(bytes))
    }
}

impl From<PhpString> for Value {
    fn from(s: PhpString) -> Self {
        Value::Str(s)
    }
}

impl From<PhpArray> for Value {
    fn from(a: PhpArray) -> Self {
        Value::Array(a)
    }
}

impl From<Number> for Value {
    fn from(n: Number) -> Self {
        match n {
            Number::Int(i) => Value::Int(i),
            Number::Float(f) => Value::Float(f),
        }
    }
}
