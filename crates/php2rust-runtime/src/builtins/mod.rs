//! PHP's built-in functions.
//!
//! Each function takes its arguments as a slice plus the request context.
//! Functions whose first parameter is by-reference (`sort`, `array_push`, ...)
//! receive it as `&mut Value` and the remaining arguments as a slice.
//! [`BUILTINS`] tells the transpiler which functions exist and their arity.

mod arrays;
mod math;
mod strings;

use std::f64::consts::{E, PI, SQRT_2};

pub use arrays::*;
pub use math::*;
pub use strings::*;

use crate::context::Context;
use crate::error::{PhpError, PhpResult};
use crate::numeric::{self, Number, Numericity};
use crate::string::PhpString;
use crate::{PhpArray, Value, format, rt};

/// How the transpiler should call a builtin.
pub struct Builtin {
    /// The PHP function name, in lowercase.
    pub name: &'static str,
    /// Calls with fewer arguments are refused at conversion time.
    pub min_args: usize,
    /// `None` for variadic functions.
    pub max_args: Option<usize>,
    /// The first argument is passed by reference.
    pub by_ref_first: bool,
}

const fn f(name: &'static str, min_args: usize, max_args: usize) -> Builtin {
    Builtin {
        name,
        min_args,
        max_args: Some(max_args),
        by_ref_first: false,
    }
}

const fn variadic(name: &'static str, min_args: usize) -> Builtin {
    Builtin {
        name,
        min_args,
        max_args: None,
        by_ref_first: false,
    }
}

const fn by_ref(name: &'static str, min_args: usize, max_args: Option<usize>) -> Builtin {
    Builtin {
        name,
        min_args,
        max_args,
        by_ref_first: true,
    }
}

/// Every builtin the transpiler can call, with its arity.
pub const BUILTINS: &[Builtin] = &[
    // output
    variadic("var_dump", 1),
    f("print_r", 1, 2),
    variadic("printf", 1),
    variadic("sprintf", 1),
    f("number_format", 1, 4),
    // strings
    f("strlen", 1, 1),
    f("strtoupper", 1, 1),
    f("strtolower", 1, 1),
    f("ucfirst", 1, 1),
    f("lcfirst", 1, 1),
    f("ucwords", 1, 2),
    f("str_repeat", 2, 2),
    f("substr", 2, 3),
    f("strpos", 2, 3),
    f("stripos", 2, 3),
    f("strrpos", 2, 3),
    f("str_contains", 2, 2),
    f("str_starts_with", 2, 2),
    f("str_ends_with", 2, 2),
    f("str_replace", 3, 3),
    f("trim", 1, 2),
    f("ltrim", 1, 2),
    f("rtrim", 1, 2),
    f("implode", 1, 2),
    f("join", 1, 2),
    f("explode", 2, 3),
    f("strrev", 1, 1),
    f("str_pad", 2, 4),
    f("str_split", 1, 2),
    f("htmlspecialchars", 1, 4),
    f("nl2br", 1, 2),
    f("ord", 1, 1),
    f("chr", 1, 1),
    f("strcmp", 2, 2),
    // types
    f("gettype", 1, 1),
    f("intval", 1, 2),
    f("floatval", 1, 1),
    f("strval", 1, 1),
    f("boolval", 1, 1),
    f("is_int", 1, 1),
    f("is_integer", 1, 1),
    f("is_float", 1, 1),
    f("is_string", 1, 1),
    f("is_bool", 1, 1),
    f("is_array", 1, 1),
    f("is_null", 1, 1),
    f("is_numeric", 1, 1),
    f("is_scalar", 1, 1),
    // math
    f("abs", 1, 1),
    variadic("max", 1),
    variadic("min", 1),
    f("floor", 1, 1),
    f("ceil", 1, 1),
    f("round", 1, 2),
    f("intdiv", 2, 2),
    f("fmod", 2, 2),
    f("sqrt", 1, 1),
    f("pow", 2, 2),
    f("pi", 0, 0),
    // arrays
    f("count", 1, 2),
    f("sizeof", 1, 2),
    f("array_keys", 1, 3),
    f("array_values", 1, 1),
    f("in_array", 2, 3),
    f("array_search", 2, 3),
    f("array_key_exists", 2, 2),
    f("key_exists", 2, 2),
    variadic("array_merge", 0),
    f("array_reverse", 1, 2),
    f("array_slice", 2, 4),
    f("range", 2, 3),
    f("array_sum", 1, 1),
    f("array_product", 1, 1),
    f("array_flip", 1, 1),
    f("array_unique", 1, 2),
    f("array_fill", 3, 3),
    f("array_combine", 2, 2),
    f("array_key_first", 1, 1),
    f("array_key_last", 1, 1),
    by_ref("sort", 1, Some(2)),
    by_ref("rsort", 1, Some(2)),
    by_ref("asort", 1, Some(2)),
    by_ref("arsort", 1, Some(2)),
    by_ref("ksort", 1, Some(2)),
    by_ref("krsort", 1, Some(2)),
    by_ref("array_push", 1, None),
    by_ref("array_pop", 1, Some(1)),
    by_ref("array_shift", 1, Some(1)),
    by_ref("array_unshift", 1, None),
    // constants
    f("define", 2, 2),
    f("defined", 1, 1),
    f("constant", 1, 1),
];

/// Looks up a builtin by its (case-insensitive) PHP name.
pub fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name.eq_ignore_ascii_case(name))
}

/// A predefined constant's value, inlined by the transpiler.
#[derive(Clone, Copy, Debug)]
pub enum Constant {
    /// An integer constant such as `PHP_INT_MAX`.
    Int(i64),
    /// A float constant such as `M_PI`.
    Float(f64),
    /// A string constant such as `PHP_EOL`.
    Str(&'static str),
}

/// Every predefined constant, by name.
pub const CONSTANTS: &[(&str, Constant)] = &[
    ("PHP_EOL", Constant::Str("\n")),
    ("PHP_INT_MAX", Constant::Int(i64::MAX)),
    ("PHP_INT_MIN", Constant::Int(i64::MIN)),
    ("PHP_INT_SIZE", Constant::Int(8)),
    ("PHP_FLOAT_EPSILON", Constant::Float(f64::EPSILON)),
    ("PHP_FLOAT_MAX", Constant::Float(f64::MAX)),
    ("PHP_FLOAT_MIN", Constant::Float(f64::MIN_POSITIVE)),
    ("NAN", Constant::Float(f64::NAN)),
    ("INF", Constant::Float(f64::INFINITY)),
    ("M_PI", Constant::Float(PI)),
    ("M_E", Constant::Float(E)),
    ("M_SQRT2", Constant::Float(SQRT_2)),
    ("DIRECTORY_SEPARATOR", Constant::Str("/")),
    ("E_ERROR", Constant::Int(1)),
    ("E_WARNING", Constant::Int(2)),
    ("E_NOTICE", Constant::Int(8)),
    ("E_DEPRECATED", Constant::Int(8192)),
    ("E_ALL", Constant::Int(32767)),
    ("STR_PAD_LEFT", Constant::Int(0)),
    ("STR_PAD_RIGHT", Constant::Int(1)),
    ("STR_PAD_BOTH", Constant::Int(2)),
    ("COUNT_NORMAL", Constant::Int(0)),
    ("COUNT_RECURSIVE", Constant::Int(1)),
    ("SORT_REGULAR", Constant::Int(0)),
    ("SORT_NUMERIC", Constant::Int(1)),
    ("SORT_STRING", Constant::Int(2)),
    ("ENT_HTML_QUOTE_SINGLE", Constant::Int(1)),
    ("ENT_COMPAT", Constant::Int(2)),
    ("ENT_QUOTES", Constant::Int(3)),
    ("ENT_NOQUOTES", Constant::Int(0)),
    ("ENT_IGNORE", Constant::Int(4)),
    ("ENT_SUBSTITUTE", Constant::Int(8)),
    ("ENT_HTML401", Constant::Int(0)),
    ("ENT_XML1", Constant::Int(16)),
    ("ENT_XHTML", Constant::Int(32)),
    ("ENT_HTML5", Constant::Int(48)),
    ("PHP_ROUND_HALF_UP", Constant::Int(1)),
];

/// Looks up a predefined constant. Constant names are case-sensitive.
pub fn predefined_constant(name: &str) -> Option<Constant> {
    CONSTANTS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

// ------------------------------------------------------ argument helpers

fn type_error(func: &str, i: usize, name: &str, expected: &str, given: &Value) -> PhpError {
    PhpError::type_error(format!(
        "{func}(): Argument #{} (${name}) must be of type {expected}, {} given",
        i + 1,
        given.type_name()
    ))
}

fn null_deprecated(func: &str, i: usize, name: &str, ty: &str, ctx: &mut Context) {
    ctx.deprecated(format!(
        "{func}(): Passing null to parameter #{} (${name}) of type {ty} is deprecated",
        i + 1
    ));
}

pub(crate) fn str_arg(
    func: &str,
    args: &[Value],
    i: usize,
    name: &str,
    ctx: &mut Context,
) -> PhpResult<PhpString> {
    match args.get(i) {
        None => Ok(PhpString::default()),
        Some(Value::Null) => {
            null_deprecated(func, i, name, "string", ctx);
            Ok(PhpString::default())
        }
        Some(v @ Value::Array(_)) => Err(type_error(func, i, name, "string", v)),
        Some(v) => Ok(v.to_php_string()),
    }
}

pub(crate) fn number_arg(
    func: &str,
    args: &[Value],
    i: usize,
    name: &str,
    ctx: &mut Context,
) -> PhpResult<Number> {
    match args.get(i) {
        None => Ok(Number::Int(0)),
        Some(Value::Null) => {
            null_deprecated(func, i, name, "int|float", ctx);
            Ok(Number::Int(0))
        }
        Some(Value::Int(n)) => Ok(Number::Int(*n)),
        Some(Value::Float(n)) => Ok(Number::Float(*n)),
        Some(Value::Bool(b)) => Ok(Number::Int(*b as i64)),
        Some(v @ Value::Str(s)) => match numeric::classify(s.as_bytes()) {
            Numericity::Numeric(n) => Ok(n),
            _ => Err(type_error(func, i, name, "int|float", v)),
        },
        Some(v) => Err(type_error(func, i, name, "int|float", v)),
    }
}

pub(crate) fn int_arg(
    func: &str,
    args: &[Value],
    i: usize,
    name: &str,
    ctx: &mut Context,
) -> PhpResult<i64> {
    match number_arg(func, args, i, name, ctx) {
        Ok(Number::Int(n)) => Ok(n),
        Ok(Number::Float(f)) if f.is_finite() => {
            if f.fract() != 0.0 {
                ctx.deprecated(format!(
                    "Implicit conversion from float {} to int loses precision",
                    format::float_repr(f)
                ));
            }
            Ok(numeric::float_to_int(f))
        }
        Ok(Number::Float(_)) => Err(type_error(func, i, name, "int", &args[i])),
        Err(_) => Err(type_error(func, i, name, "int", &args[i])),
    }
}

pub(crate) fn float_arg(
    func: &str,
    args: &[Value],
    i: usize,
    name: &str,
    ctx: &mut Context,
) -> PhpResult<f64> {
    number_arg(func, args, i, name, ctx)
        .map(Number::as_f64)
        .map_err(|_| type_error(func, i, name, "float", &args[i]))
}

pub(crate) fn bool_arg(func: &str, args: &[Value], i: usize, name: &str) -> PhpResult<bool> {
    match args.get(i) {
        None => Ok(false),
        Some(v @ Value::Array(_)) => Err(type_error(func, i, name, "bool", v)),
        Some(v) => Ok(v.to_bool()),
    }
}

pub(crate) fn array_arg<'a>(
    func: &str,
    args: &'a [Value],
    i: usize,
    name: &str,
) -> PhpResult<&'a PhpArray> {
    match args.get(i) {
        Some(Value::Array(a)) => Ok(a),
        Some(v) => Err(type_error(func, i, name, "array", v)),
        None => Err(type_error(func, i, name, "array", &Value::Null)),
    }
}

/// The by-reference array parameter of `sort`, `array_push`, ...
pub(crate) fn array_target<'a>(
    func: &str,
    target: &'a mut Value,
    name: &str,
) -> PhpResult<&'a mut PhpArray> {
    match target {
        Value::Array(a) => Ok(a),
        other => Err(type_error(func, 0, name, "array", other)),
    }
}

// ---------------------------------------------------------------- output

/// PHP's [`var_dump()`](https://www.php.net/manual/en/function.var-dump.php).
pub fn var_dump(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut out = Vec::new();
    for v in args {
        format::var_dump(v, 0, &mut out);
    }
    ctx.write(&out);
    Ok(Value::Null)
}

/// PHP's [`print_r()`](https://www.php.net/manual/en/function.print-r.php).
pub fn print_r(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut out = Vec::new();
    format::print_r(&args[0], 0, &mut out);
    if bool_arg("print_r", args, 1, "return")? {
        return Ok(Value::from(out));
    }
    ctx.write(&out);
    Ok(Value::Bool(true))
}

// ------------------------------------------------------------- constants

/// PHP's [`define()`](https://www.php.net/manual/en/function.define.php).
pub fn define(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let name = str_arg("define", args, 0, "constant_name", ctx)?;
    Ok(Value::Bool(ctx.define(name.as_bytes(), args[1].clone())))
}

/// PHP's [`defined()`](https://www.php.net/manual/en/function.defined.php).
pub fn defined(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let name = str_arg("defined", args, 0, "constant_name", ctx)?;
    let known = ctx.constant(name.as_bytes()).is_ok()
        || predefined_constant(&name.to_string_lossy()).is_some();
    Ok(Value::Bool(known))
}

/// PHP's [`constant()`](https://www.php.net/manual/en/function.constant.php).
pub fn constant(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let name = str_arg("constant", args, 0, "name", ctx)?;
    match predefined_constant(&name.to_string_lossy()) {
        Some(Constant::Int(i)) => Ok(Value::Int(i)),
        Some(Constant::Float(f)) => Ok(Value::Float(f)),
        Some(Constant::Str(s)) => Ok(Value::from(s)),
        None => ctx.constant(name.as_bytes()),
    }
}

// ----------------------------------------------------------------- types

/// PHP's [`gettype()`](https://www.php.net/manual/en/function.gettype.php).
pub fn gettype(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::from(args[0].gettype()))
}

/// PHP's [`intval()`](https://www.php.net/manual/en/function.intval.php).
pub fn intval(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let base = if args.len() > 1 {
        int_arg("intval", args, 1, "base", ctx)?
    } else {
        10
    };
    match (&args[0], base) {
        (_, 10) => Ok(Value::Int(args[0].to_int())),
        (Value::Str(s), 0 | 2 | 8 | 16) => {
            let text = s.to_string_lossy();
            let t = text.trim_start();
            let (neg, t) = match t.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, t.strip_prefix('+').unwrap_or(t)),
            };
            let lower = t.to_ascii_lowercase();
            let (radix, digits) = match base {
                16 => (16, lower.strip_prefix("0x").unwrap_or(&lower)),
                8 => (8, lower.strip_prefix("0o").unwrap_or(&lower)),
                2 => (2, lower.strip_prefix("0b").unwrap_or(&lower)),
                _ if lower.starts_with("0x") => (16, &lower[2..]),
                _ if lower.starts_with("0b") => (2, &lower[2..]),
                _ if lower.starts_with("0o") => (8, &lower[2..]),
                _ if lower.starts_with('0') && lower.len() > 1 => (8, &lower[1..]),
                _ => (10, lower.as_str()),
            };
            let end = digits
                .find(|c: char| !c.is_digit(radix))
                .unwrap_or(digits.len());
            let n = i64::from_str_radix(&digits[..end], radix).unwrap_or(0);
            Ok(Value::Int(if neg { -n } else { n }))
        }
        _ => Ok(Value::Int(args[0].to_int())),
    }
}

/// PHP's [`floatval()`](https://www.php.net/manual/en/function.floatval.php).
pub fn floatval(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Float(args[0].to_float()))
}

/// PHP's [`strval()`](https://www.php.net/manual/en/function.strval.php).
pub fn strval(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(rt::cast_string(&args[0], ctx))
}

/// PHP's [`boolval()`](https://www.php.net/manual/en/function.boolval.php).
pub fn boolval(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(args[0].to_bool()))
}

/// PHP's [`is_int()`](https://www.php.net/manual/en/function.is-int.php).
pub fn is_int(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(args[0], Value::Int(_))))
}

/// PHP's [`is_integer()`](https://www.php.net/manual/en/function.is-integer.php).
pub fn is_integer(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    is_int(args, ctx)
}

/// PHP's [`is_float()`](https://www.php.net/manual/en/function.is-float.php).
pub fn is_float(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(args[0], Value::Float(_))))
}

/// PHP's [`is_string()`](https://www.php.net/manual/en/function.is-string.php).
pub fn is_string(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(args[0], Value::Str(_))))
}

/// PHP's [`is_bool()`](https://www.php.net/manual/en/function.is-bool.php).
pub fn is_bool(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(args[0], Value::Bool(_))))
}

/// PHP's [`is_array()`](https://www.php.net/manual/en/function.is-array.php).
pub fn is_array(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(args[0], Value::Array(_))))
}

/// PHP's [`is_null()`](https://www.php.net/manual/en/function.is-null.php).
pub fn is_null(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(args[0].is_null()))
}

/// PHP's [`is_scalar()`](https://www.php.net/manual/en/function.is-scalar.php).
pub fn is_scalar(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(matches!(
        args[0],
        Value::Int(_) | Value::Float(_) | Value::Str(_) | Value::Bool(_)
    )))
}

/// PHP's [`is_numeric()`](https://www.php.net/manual/en/function.is-numeric.php).
pub fn is_numeric(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Bool(match &args[0] {
        Value::Int(_) | Value::Float(_) => true,
        Value::Str(s) => numeric::numeric(s.as_bytes()).is_some(),
        _ => false,
    }))
}
