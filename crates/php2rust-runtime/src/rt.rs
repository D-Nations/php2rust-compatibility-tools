//! Operations that generated code calls: PHP's operators, conversions, array
//! access and calling conventions. `ctx` comes last so that operand
//! expressions (which may themselves use `ctx`) are evaluated before it is
//! borrowed.

use std::cmp::Ordering;

use crate::array::{Key, PhpArray};
use crate::context::Context;
use crate::error::{PhpError, PhpResult};
use crate::format;
use crate::numeric::{self, Number, Numericity};
use crate::string::PhpString;
use crate::value::Value;

// ---------------------------------------------------------------- output

pub fn echo(value: &Value, ctx: &mut Context) -> PhpResult<()> {
    let s = to_str(value, ctx);
    ctx.write(s.as_bytes());
    Ok(())
}

pub fn echo_bytes(bytes: &[u8], ctx: &mut Context) {
    ctx.write(bytes);
}

/// `print`: echoes and evaluates to 1.
pub fn print(value: &Value, ctx: &mut Context) -> PhpResult<Value> {
    echo(value, ctx)?;
    Ok(Value::Int(1))
}

/// `exit` / `die`. A string argument is printed; an int is the exit status.
pub fn exit(arg: Option<&Value>, ctx: &mut Context) -> PhpResult<Value> {
    let code = match arg {
        Some(Value::Int(code)) => *code as i32,
        Some(other) => {
            echo(other, ctx)?;
            0
        }
        None => 0,
    };
    Err(PhpError::Exit(code))
}

// ----------------------------------------------------------- conversions

/// String conversion as PHP does it in string contexts, including the
/// "Array to string conversion" warning.
pub fn to_str(value: &Value, ctx: &mut Context) -> PhpString {
    if let Value::Array(_) = value {
        ctx.warn("Array to string conversion");
    }
    value.to_php_string()
}

pub fn cast_string(value: &Value, ctx: &mut Context) -> Value {
    Value::Str(to_str(value, ctx))
}

pub fn cast_array(value: Value) -> Value {
    match value {
        Value::Array(_) => value,
        Value::Null => Value::Array(PhpArray::new()),
        scalar => Value::Array(PhpArray::from_list([scalar])),
    }
}

/// Builds interpolated strings (`"Hello $name"`).
#[derive(Default)]
pub struct StrBuilder(Vec<u8>);

impl StrBuilder {
    pub fn new() -> Self {
        StrBuilder::default()
    }

    pub fn lit(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    pub fn val(&mut self, value: &Value, ctx: &mut Context) {
        let s = to_str(value, ctx);
        self.0.extend_from_slice(s.as_bytes());
    }

    pub fn finish(self) -> Value {
        Value::from(self.0)
    }
}

// ------------------------------------------------------------ arithmetic

fn unsupported_operands(op: &str, a: &Value, b: &Value) -> PhpError {
    PhpError::type_error(format!(
        "Unsupported operand types: {} {op} {}",
        a.type_name(),
        b.type_name()
    ))
}

/// Converts an arithmetic operand to a number. Per the "Saner numeric
/// strings" RFC, leading-numeric strings warn and non-numeric ones throw.
fn operand(v: &Value, op: &str, a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Number> {
    Ok(match v {
        Value::Null => Number::Int(0),
        Value::Bool(b) => Number::Int(*b as i64),
        Value::Int(i) => Number::Int(*i),
        Value::Float(f) => Number::Float(*f),
        Value::Str(s) => match numeric::classify(s.as_bytes()) {
            Numericity::Numeric(n) => n,
            Numericity::Leading(n) => {
                ctx.warn("A non-numeric value encountered");
                n
            }
            Numericity::NonNumeric => return Err(unsupported_operands(op, a, b)),
        },
        Value::Array(_) => return Err(unsupported_operands(op, a, b)),
    })
}

fn operands(op: &str, a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<(Number, Number)> {
    Ok((operand(a, op, a, b, ctx)?, operand(b, op, a, b, ctx)?))
}

fn int_or_float(
    x: Number,
    y: Number,
    int_op: fn(i64, i64) -> Option<i64>,
    float_op: fn(f64, f64) -> f64,
) -> Value {
    match (x, y) {
        (Number::Int(i), Number::Int(j)) => match int_op(i, j) {
            Some(r) => Value::Int(r),
            None => Value::Float(float_op(i as f64, j as f64)),
        },
        _ => Value::Float(float_op(x.as_f64(), y.as_f64())),
    }
}

pub fn add(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    if let (Value::Array(x), Value::Array(y)) = (a, b) {
        let mut union = x.clone();
        for (k, v) in y.iter() {
            if !union.contains_key(k) {
                union.insert(k.clone(), v.clone());
            }
        }
        return Ok(Value::Array(union));
    }
    let (x, y) = operands("+", a, b, ctx)?;
    Ok(int_or_float(x, y, i64::checked_add, |p, q| p + q))
}

pub fn sub(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (x, y) = operands("-", a, b, ctx)?;
    Ok(int_or_float(x, y, i64::checked_sub, |p, q| p - q))
}

pub fn mul(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (x, y) = operands("*", a, b, ctx)?;
    Ok(int_or_float(x, y, i64::checked_mul, |p, q| p * q))
}

pub fn div(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (x, y) = operands("/", a, b, ctx)?;
    if y.as_f64() == 0.0 {
        return Err(PhpError::division_by_zero("Division by zero"));
    }
    Ok(match (x, y) {
        (Number::Int(i), Number::Int(j)) if i.checked_rem(j) == Some(0) => Value::Int(i / j),
        _ => Value::Float(x.as_f64() / y.as_f64()),
    })
}

fn int_operand(n: Number, ctx: &mut Context) -> i64 {
    match n {
        Number::Int(i) => i,
        Number::Float(f) => {
            if f.fract() != 0.0 && f.is_finite() {
                ctx.deprecated(format!(
                    "Implicit conversion from float {} to int loses precision",
                    format::float_repr(f)
                ));
            }
            numeric::float_to_int(f)
        }
    }
}

pub fn rem(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (x, y) = operands("%", a, b, ctx)?;
    let (i, j) = (int_operand(x, ctx), int_operand(y, ctx));
    if j == 0 {
        return Err(PhpError::division_by_zero("Modulo by zero"));
    }
    Ok(Value::Int(i.wrapping_rem(j)))
}

pub fn pow(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (x, y) = operands("**", a, b, ctx)?;
    Ok(match (x, y) {
        (Number::Int(i), Number::Int(j)) if j >= 0 => {
            match u32::try_from(j).ok().and_then(|j| i.checked_pow(j)) {
                Some(r) => Value::Int(r),
                None => Value::Float((i as f64).powf(j as f64)),
            }
        }
        _ => Value::Float(x.as_f64().powf(y.as_f64())),
    })
}

/// Unary minus. PHP compiles `-$x` as `$x * -1`.
pub fn neg(a: &Value, ctx: &mut Context) -> PhpResult<Value> {
    mul(a, &Value::Int(-1), ctx)
}

/// Unary plus. PHP compiles `+$x` as `$x * 1`.
pub fn plus(a: &Value, ctx: &mut Context) -> PhpResult<Value> {
    mul(a, &Value::Int(1), ctx)
}

fn int_operands(op: &str, a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<(i64, i64)> {
    let (x, y) = operands(op, a, b, ctx)?;
    Ok((int_operand(x, ctx), int_operand(y, ctx)))
}

fn bytewise(a: &PhpString, b: &PhpString, op: fn(u8, u8) -> u8, longest: bool) -> Value {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let len = if longest {
        x.len().max(y.len())
    } else {
        x.len().min(y.len())
    };
    let byte = |s: &[u8], i: usize| s.get(i).copied().unwrap_or(0);
    Value::from(
        (0..len)
            .map(|i| op(byte(x, i), byte(y, i)))
            .collect::<Vec<u8>>(),
    )
}

pub fn bit_and(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    if let (Value::Str(x), Value::Str(y)) = (a, b) {
        return Ok(bytewise(x, y, |p, q| p & q, false));
    }
    let (i, j) = int_operands("&", a, b, ctx)?;
    Ok(Value::Int(i & j))
}

pub fn bit_or(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    if let (Value::Str(x), Value::Str(y)) = (a, b) {
        return Ok(bytewise(x, y, |p, q| p | q, true));
    }
    let (i, j) = int_operands("|", a, b, ctx)?;
    Ok(Value::Int(i | j))
}

pub fn bit_xor(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    if let (Value::Str(x), Value::Str(y)) = (a, b) {
        return Ok(bytewise(x, y, |p, q| p ^ q, false));
    }
    let (i, j) = int_operands("^", a, b, ctx)?;
    Ok(Value::Int(i ^ j))
}

pub fn shl(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (i, j) = int_operands("<<", a, b, ctx)?;
    if j < 0 {
        return Err(PhpError::arithmetic_error("Bit shift by negative number"));
    }
    Ok(Value::Int(if j >= 64 { 0 } else { i << j }))
}

pub fn shr(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let (i, j) = int_operands(">>", a, b, ctx)?;
    if j < 0 {
        return Err(PhpError::arithmetic_error("Bit shift by negative number"));
    }
    Ok(Value::Int(if j >= 64 {
        if i < 0 { -1 } else { 0 }
    } else {
        i >> j
    }))
}

pub fn bit_not(a: &Value) -> PhpResult<Value> {
    match a {
        Value::Int(i) => Ok(Value::Int(!i)),
        Value::Float(f) => Ok(Value::Int(!numeric::float_to_int(*f))),
        Value::Str(s) => Ok(Value::from(
            s.as_bytes().iter().map(|b| !b).collect::<Vec<u8>>(),
        )),
        other => Err(PhpError::type_error(format!(
            "Cannot perform bitwise not on {}",
            other.type_name()
        ))),
    }
}

pub fn concat(a: &Value, b: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let mut bytes = to_str(a, ctx).as_bytes().to_vec();
    bytes.extend_from_slice(to_str(b, ctx).as_bytes());
    Ok(Value::from(bytes))
}

/// `$s .= $x`, appending in place when the string is not shared.
pub fn concat_assign(target: &mut Value, rhs: &Value, ctx: &mut Context) -> PhpResult<Value> {
    let suffix = to_str(rhs, ctx);
    if !matches!(target, Value::Str(_)) {
        *target = Value::Str(to_str(target, ctx));
    }
    if let Value::Str(s) = target {
        s.make_mut().extend_from_slice(suffix.as_bytes());
    }
    Ok(target.clone())
}

// ----------------------------------------------------------- comparison

fn cmp_f64(x: f64, y: f64) -> Ordering {
    // ZEND_THREEWAY_COMPARE: anything involving NaN compares as "greater".
    if x == y {
        Ordering::Equal
    } else if x < y {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

fn cmp_numbers(x: Number, y: Number) -> Ordering {
    match (x, y) {
        (Number::Int(i), Number::Int(j)) => i.cmp(&j),
        _ => cmp_f64(x.as_f64(), y.as_f64()),
    }
}

fn number_bytes(n: Number) -> Vec<u8> {
    match n {
        Number::Int(i) => i.to_string().into_bytes(),
        Number::Float(f) => format::float_to_string(f).into_bytes(),
    }
}

/// PHP 8: a number equals a string only if the string is numeric; otherwise
/// the number is compared as a string.
fn cmp_number_string(n: Number, s: &[u8]) -> Ordering {
    match numeric::numeric(s) {
        Some(m) => cmp_numbers(n, m),
        None => number_bytes(n).as_slice().cmp(s),
    }
}

fn cmp_strings(a: &[u8], b: &[u8]) -> Ordering {
    match (numeric::numeric(a), numeric::numeric(b)) {
        (Some(x), Some(y)) => cmp_numbers(x, y),
        _ => a.cmp(b),
    }
}

fn cmp_arrays(a: &PhpArray, b: &PhpArray) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for (k, v) in a.iter() {
        match b.get(k) {
            // Uncomparable: zend_compare reports 1.
            None => return Ordering::Greater,
            Some(w) => match compare(v, w) {
                Ordering::Equal => {}
                other => return other,
            },
        }
    }
    Ordering::Equal
}

/// PHP 8's `zend_compare` (the `<=>` operator).
pub fn compare(a: &Value, b: &Value) -> Ordering {
    use Value::*;
    match (a, b) {
        (Int(i), Int(j)) => i.cmp(j),
        (Int(i), Float(f)) => cmp_f64(*i as f64, *f),
        (Float(f), Int(i)) => cmp_f64(*f, *i as f64),
        (Float(f), Float(g)) => cmp_f64(*f, *g),
        (Array(x), Array(y)) => cmp_arrays(x, y),
        (Null, Null) => Ordering::Equal,
        (Bool(x), Bool(y)) => x.cmp(y),
        (Null, Bool(y)) => false.cmp(y),
        (Bool(x), Null) => x.cmp(&false),
        (Str(x), Str(y)) => cmp_strings(x.as_bytes(), y.as_bytes()),
        (Null, Str(s)) => b"".as_slice().cmp(s.as_bytes()),
        (Str(s), Null) => s.as_bytes().cmp(b"".as_slice()),
        (Int(i), Str(s)) => cmp_number_string(Number::Int(*i), s.as_bytes()),
        (Float(f), Str(s)) => cmp_number_string(Number::Float(*f), s.as_bytes()),
        (Str(s), Int(i)) => cmp_number_string(Number::Int(*i), s.as_bytes()).reverse(),
        (Str(s), Float(f)) => cmp_number_string(Number::Float(*f), s.as_bytes()).reverse(),
        (Null | Bool(_), _) | (_, Null | Bool(_)) => a.to_bool().cmp(&b.to_bool()),
        (Array(_), _) => Ordering::Greater,
        (_, Array(_)) => Ordering::Less,
    }
}

/// `==`
pub fn loose_eq(a: &Value, b: &Value) -> bool {
    compare(a, b) == Ordering::Equal
}

/// `===`
pub fn strict_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Bool(x), Bool(y)) => x == y,
        (Int(x), Int(y)) => x == y,
        (Float(x), Float(y)) => x == y,
        (Str(x), Str(y)) => x == y,
        (Array(x), Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((k1, v1), (k2, v2))| k1 == k2 && strict_eq(v1, v2))
        }
        _ => false,
    }
}

pub fn lt(a: &Value, b: &Value) -> bool {
    compare(a, b) == Ordering::Less
}

pub fn le(a: &Value, b: &Value) -> bool {
    compare(a, b) != Ordering::Greater
}

/// PHP compiles `a > b` as `b < a`, which matters for NaN and uncomparable arrays.
pub fn gt(a: &Value, b: &Value) -> bool {
    compare(b, a) == Ordering::Less
}

pub fn ge(a: &Value, b: &Value) -> bool {
    compare(b, a) != Ordering::Greater
}

pub fn spaceship(a: &Value, b: &Value) -> Value {
    Value::Int(compare(a, b) as i64)
}

// ---------------------------------------------------- increment/decrement

fn increment_string(s: &[u8]) -> Vec<u8> {
    #[derive(Clone, Copy)]
    enum Last {
        Lower,
        Upper,
        Digit,
    }
    if s.is_empty() {
        return b"1".to_vec();
    }
    let mut out = s.to_vec();
    let mut last = Last::Digit;
    let mut carry = false;
    for pos in (0..out.len()).rev() {
        let (wrapped, first, kind) = match out[pos] {
            b'a'..=b'z' => (out[pos] == b'z', b'a', Last::Lower),
            b'A'..=b'Z' => (out[pos] == b'Z', b'A', Last::Upper),
            b'0'..=b'9' => (out[pos] == b'9', b'0', Last::Digit),
            _ => {
                carry = false;
                break;
            }
        };
        last = kind;
        if wrapped {
            out[pos] = first;
            carry = true;
        } else {
            out[pos] += 1;
            carry = false;
            break;
        }
    }
    if carry {
        let prefix = match last {
            Last::Lower => b'a',
            Last::Upper => b'A',
            Last::Digit => b'1',
        };
        out.insert(0, prefix);
    }
    out
}

pub fn inc(v: &mut Value) -> PhpResult<()> {
    *v = match &*v {
        Value::Null => Value::Int(1),
        Value::Int(i) => i
            .checked_add(1)
            .map_or(Value::Float(*i as f64 + 1.0), Value::Int),
        Value::Float(f) => Value::Float(f + 1.0),
        Value::Bool(_) => return Ok(()),
        Value::Str(s) => match numeric::numeric(s.as_bytes()) {
            Some(Number::Int(i)) => i
                .checked_add(1)
                .map_or(Value::Float(i as f64 + 1.0), Value::Int),
            Some(Number::Float(f)) => Value::Float(f + 1.0),
            None => Value::from(increment_string(s.as_bytes())),
        },
        Value::Array(_) => return Err(PhpError::type_error("Cannot increment array")),
    };
    Ok(())
}

pub fn dec(v: &mut Value) -> PhpResult<()> {
    *v = match &*v {
        Value::Null | Value::Bool(_) => return Ok(()),
        Value::Int(i) => i
            .checked_sub(1)
            .map_or(Value::Float(*i as f64 - 1.0), Value::Int),
        Value::Float(f) => Value::Float(f - 1.0),
        Value::Str(s) if s.is_empty() => Value::Int(-1),
        Value::Str(s) => match numeric::numeric(s.as_bytes()) {
            Some(Number::Int(i)) => i
                .checked_sub(1)
                .map_or(Value::Float(i as f64 - 1.0), Value::Int),
            Some(Number::Float(f)) => Value::Float(f - 1.0),
            None => return Ok(()),
        },
        Value::Array(_) => return Err(PhpError::type_error("Cannot decrement array")),
    };
    Ok(())
}

pub fn pre_inc(v: &mut Value) -> PhpResult<Value> {
    inc(v)?;
    Ok(v.clone())
}

pub fn pre_dec(v: &mut Value) -> PhpResult<Value> {
    dec(v)?;
    Ok(v.clone())
}

pub fn post_inc(v: &mut Value) -> PhpResult<Value> {
    let old = v.clone();
    inc(v)?;
    Ok(old)
}

pub fn post_dec(v: &mut Value) -> PhpResult<Value> {
    let old = v.clone();
    dec(v)?;
    Ok(old)
}

// ---------------------------------------------------------- array access

/// Converts a value to an array key the way `$a[$k]` does.
pub fn key(k: &Value, ctx: &mut Context) -> PhpResult<Key> {
    Ok(match k {
        Value::Int(i) => Key::Int(*i),
        Value::Str(s) => Key::from_bytes(s.as_bytes()),
        Value::Bool(b) => Key::Int(*b as i64),
        Value::Null => Key::Str(PhpString::default()),
        Value::Float(f) => Key::Int(int_operand(Number::Float(*f), ctx)),
        Value::Array(_) => {
            return Err(PhpError::type_error(
                "Cannot access offset of type array on array",
            ));
        }
    })
}

fn describe_key(k: &Key) -> String {
    match k {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => format!("\"{}\"", s.to_string_lossy()),
    }
}

fn string_offset(s: &PhpString, k: &Value, ctx: &mut Context) -> PhpResult<Option<usize>> {
    let i = match k {
        Value::Int(i) => *i,
        Value::Bool(b) => *b as i64,
        Value::Float(f) => numeric::float_to_int(*f),
        Value::Str(o) => match numeric::numeric(o.as_bytes()) {
            Some(Number::Int(i)) => i,
            _ => {
                return Err(PhpError::type_error(format!(
                    "Cannot access offset of type {} on string",
                    k.type_name()
                )));
            }
        },
        _ => {
            return Err(PhpError::type_error(format!(
                "Cannot access offset of type {} on string",
                k.type_name()
            )));
        }
    };
    let len = s.len() as i64;
    let idx = if i < 0 { len + i } else { i };
    if idx < 0 || idx >= len {
        ctx.warn(format!("Uninitialized string offset {i}"));
        return Ok(None);
    }
    Ok(Some(idx as usize))
}

/// Reads `$base[$k]`.
pub fn dim_get(base: &Value, k: &Value, ctx: &mut Context) -> PhpResult<Value> {
    match base {
        Value::Array(a) => {
            let key = key(k, ctx)?;
            match a.get(&key) {
                Some(v) => Ok(v.clone()),
                None => {
                    ctx.warn(format!("Undefined array key {}", describe_key(&key)));
                    Ok(Value::Null)
                }
            }
        }
        Value::Str(s) => Ok(match string_offset(s, k, ctx)? {
            Some(i) => Value::from(vec![s.as_bytes()[i]]),
            None => Value::from(""),
        }),
        other => {
            ctx.warn(format!(
                "Trying to access array offset on {}",
                other.type_name()
            ));
            Ok(Value::Null)
        }
    }
}

/// `$base[$k]` for `isset` and `??`: never warns, `None` if missing or null.
pub fn dim_quiet(base: &Value, k: &Value) -> Option<Value> {
    match base {
        Value::Array(a) => {
            let key = match k {
                Value::Int(i) => Key::Int(*i),
                Value::Str(s) => Key::from_bytes(s.as_bytes()),
                Value::Bool(b) => Key::Int(*b as i64),
                Value::Null => Key::Str(PhpString::default()),
                Value::Float(f) => Key::Int(numeric::float_to_int(*f)),
                Value::Array(_) => return None,
            };
            a.get(&key).filter(|v| !v.is_null()).cloned()
        }
        Value::Str(s) => {
            let i = match k {
                Value::Int(i) => *i,
                Value::Str(o) => match numeric::numeric(o.as_bytes()) {
                    Some(Number::Int(i)) => i,
                    _ => return None,
                },
                _ => return None,
            };
            let idx = if i < 0 { s.len() as i64 + i } else { i };
            (idx >= 0 && (idx as usize) < s.len())
                .then(|| Value::from(vec![s.as_bytes()[idx as usize]]))
        }
        _ => None,
    }
}

fn autovivify<'a>(base: &'a mut Value, ctx: &mut Context) -> PhpResult<&'a mut PhpArray> {
    match base {
        Value::Null => *base = Value::Array(PhpArray::new()),
        Value::Bool(false) => {
            ctx.deprecated("Automatic conversion of false to array is deprecated");
            *base = Value::Array(PhpArray::new());
        }
        Value::Array(_) => {}
        Value::Str(_) => {
            return Err(PhpError::error(
                "String offsets cannot be written to by reference yet",
            ));
        }
        _ => return Err(PhpError::error("Cannot use a scalar value as an array")),
    }
    match base {
        Value::Array(a) => Ok(a),
        _ => unreachable!(),
    }
}

/// `$base[$k]` as a write target, creating arrays and elements as needed.
pub fn dim_mut<'a>(base: &'a mut Value, k: &Value, ctx: &mut Context) -> PhpResult<&'a mut Value> {
    let key = key(k, ctx)?;
    Ok(autovivify(base, ctx)?.entry_mut(key))
}

/// `$base[]` as a write target.
pub fn append_mut<'a>(base: &'a mut Value, ctx: &mut Context) -> PhpResult<&'a mut Value> {
    if let Value::Str(_) = base {
        return Err(PhpError::error("[] operator not supported for strings"));
    }
    autovivify(base, ctx)?.push_mut().ok_or_else(|| {
        PhpError::error("Cannot add element to the array as the next element is already occupied")
    })
}

/// `unset($base[$k1][$k2]...)`. Never creates anything along the path.
pub fn unset_dim(base: &mut Value, keys: &[Value], ctx: &mut Context) -> PhpResult<()> {
    let Some((last, path)) = keys.split_last() else {
        return Ok(());
    };
    let mut cur = base;
    for k in path {
        let key = key(k, ctx)?;
        match cur {
            Value::Array(a) => match a.get_mut(&key) {
                Some(next) => cur = next,
                None => return Ok(()),
            },
            _ => return Ok(()),
        }
    }
    match cur {
        Value::Array(a) => {
            let key = key(last, ctx)?;
            a.remove(&key);
            Ok(())
        }
        Value::Str(_) => Err(PhpError::error("Cannot unset string offsets")),
        _ => Ok(()),
    }
}

/// A variable read for `isset` and `??`: the value unless it is null.
pub fn present(v: &Value) -> Option<Value> {
    (!v.is_null()).then(|| v.clone())
}

/// `[...$xs]` inside an array literal. String keys are kept (PHP 8.1+).
pub fn spread(target: &mut PhpArray, v: &Value) -> PhpResult<()> {
    let Value::Array(source) = v else {
        return Err(PhpError::error(
            "Only arrays and Traversables can be unpacked",
        ));
    };
    for (k, v) in source.iter() {
        match k {
            Key::Int(_) => {
                target.push(v.clone());
            }
            Key::Str(_) => target.insert(k.clone(), v.clone()),
        }
    }
    Ok(())
}

pub fn unhandled_match(v: &Value) -> PhpError {
    let repr = match v {
        Value::Str(s) => format!("{:?}", s.to_string_lossy()),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format::float_repr(*f),
        Value::Bool(b) => b.to_string(),
        Value::Null => "NULL".into(),
        Value::Array(_) => "of type array".into(),
    };
    PhpError::unhandled_match(format!("Unhandled match case {repr}"))
}

/// The array `foreach` iterates: a cheap snapshot, so the loop body may
/// modify the original variable without affecting iteration (by-value foreach).
pub fn foreach_source(v: &Value, ctx: &mut Context) -> PhpArray {
    match v {
        Value::Array(a) => a.clone(),
        other => {
            ctx.warn(format!(
                "foreach() argument must be of type array|object, {} given",
                other.type_name()
            ));
            PhpArray::new()
        }
    }
}

// ------------------------------------------------------- function calls

/// The declared type of a parameter, for coercion at call time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
    Int,
    Float,
    String,
    Bool,
    Array,
}

impl ParamType {
    fn name(self) -> &'static str {
        match self {
            ParamType::Int => "int",
            ParamType::Float => "float",
            ParamType::String => "string",
            ParamType::Bool => "bool",
            ParamType::Array => "array",
        }
    }
}

pub fn too_few_arguments(function: &str, passed: usize, required: usize, exact: bool) -> PhpError {
    let quantifier = if exact { "exactly" } else { "at least" };
    PhpError::argument_count_error(format!(
        "Too few arguments to function {function}(), {passed} passed and {quantifier} {required} expected"
    ))
}

/// Applies a parameter type declaration, coercing scalars as PHP does in
/// the default (non-strict) mode.
pub fn coerce_param(
    v: Value,
    ty: ParamType,
    nullable: bool,
    strict: bool,
    function: &str,
    position: usize,
    name: &str,
) -> PhpResult<Value> {
    let mismatch = |v: &Value| {
        let expected = if nullable {
            format!("?{}", ty.name())
        } else {
            ty.name().to_string()
        };
        PhpError::type_error(format!(
            "{function}(): Argument #{position} (${name}) must be of type {expected}, {} given",
            v.type_name()
        ))
    };
    if v.is_null() {
        return if nullable { Ok(v) } else { Err(mismatch(&v)) };
    }
    let coerced = match (ty, &v) {
        (ParamType::Int, Value::Int(_))
        | (ParamType::Float, Value::Float(_))
        | (ParamType::String, Value::Str(_))
        | (ParamType::Bool, Value::Bool(_))
        | (ParamType::Array, Value::Array(_)) => Some(v.clone()),
        (ParamType::Float, Value::Int(i)) => Some(Value::Float(*i as f64)),
        _ if strict => None,
        (ParamType::Int, Value::Float(f)) if f.fract() == 0.0 && f.is_finite() => {
            Some(Value::Int(*f as i64))
        }
        (ParamType::Int, Value::Bool(b)) => Some(Value::Int(*b as i64)),
        (ParamType::Int, Value::Str(s)) => match numeric::numeric(s.as_bytes()) {
            Some(Number::Int(i)) => Some(Value::Int(i)),
            Some(Number::Float(f)) if f.fract() == 0.0 => Some(Value::Int(f as i64)),
            _ => None,
        },
        (ParamType::Float, Value::Bool(b)) => Some(Value::Float(*b as i64 as f64)),
        (ParamType::Float, Value::Str(s)) => {
            numeric::numeric(s.as_bytes()).map(|n| Value::Float(n.as_f64()))
        }
        (ParamType::String, Value::Int(_) | Value::Float(_) | Value::Bool(_)) => {
            Some(Value::Str(v.to_php_string()))
        }
        (ParamType::Bool, Value::Int(_) | Value::Float(_) | Value::Str(_)) => {
            Some(Value::Bool(v.to_bool()))
        }
        _ => None,
    };
    coerced.ok_or_else(|| mismatch(&v))
}

/// The variadic tail of an argument list, as a list.
pub fn rest_args(args: &[Value], from: usize) -> Value {
    Value::Array(PhpArray::from_list(args.iter().skip(from).cloned()))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn s(v: &str) -> Value {
        Value::from(v)
    }

    #[rstest]
    #[case::int_vs_text(Value::Int(0), s("a"), false)]
    #[case::numeric_strings(s("1"), s("01"), true)]
    #[case::exponent_string(s("10"), s("1e1"), true)]
    #[case::null_vs_false(Value::Null, Value::Bool(false), true)]
    #[case::empty_array_vs_false(Value::Array(PhpArray::new()), Value::Bool(false), true)]
    #[case::nan(Value::Float(f64::NAN), Value::Float(f64::NAN), false)]
    fn php8_loose_equality(#[case] a: Value, #[case] b: Value, #[case] equal: bool) {
        assert_eq!(loose_eq(&a, &b), equal);
    }

    #[test]
    fn int_overflow_becomes_float() {
        let r = add(&Value::Int(i64::MAX), &Value::Int(1), &mut Context::new()).unwrap();
        assert!(matches!(r, Value::Float(f) if f == i64::MAX as f64));
    }

    #[rstest]
    #[case::inexact(7, 2, "Float(3.5)")]
    #[case::exact(8, 2, "Int(4)")]
    fn division(#[case] a: i64, #[case] b: i64, #[case] expected: &str) {
        let r = div(&Value::Int(a), &Value::Int(b), &mut Context::new()).unwrap();
        assert_eq!(format!("{r:?}"), expected);
    }

    #[test]
    fn division_by_zero_is_an_error() {
        assert!(div(&Value::Int(1), &Value::Int(0), &mut Context::new()).is_err());
    }

    #[test]
    fn arithmetic_on_text_is_an_error() {
        assert!(add(&s("abc"), &Value::Int(1), &mut Context::new()).is_err());
    }

    #[rstest]
    #[case::letter(b"a", b"b")]
    #[case::carry_case(b"Az", b"Ba")]
    #[case::grows(b"zz", b"aaa")]
    #[case::digit(b"a9", b"b0")]
    #[case::grows_upper(b"Zz", b"AAa")]
    fn string_increment(#[case] from: &[u8], #[case] to: &[u8]) {
        assert_eq!(increment_string(from), to);
    }

    #[test]
    fn nested_autovivification() {
        let mut ctx = Context::new();
        let mut v = Value::Null;
        let slot = dim_mut(&mut v, &Value::Int(0), &mut ctx).unwrap();
        *append_mut(slot, &mut ctx).unwrap() = s("x");
        assert_eq!(
            dim_quiet(&dim_quiet(&v, &Value::Int(0)).unwrap(), &Value::Int(0))
                .unwrap()
                .to_php_string()
                .as_bytes(),
            b"x"
        );
    }
}
