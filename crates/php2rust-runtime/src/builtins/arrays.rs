use std::cmp::Ordering;

use super::{array_arg, array_target, bool_arg, int_arg, number_arg};
use crate::context::Context;
use crate::error::{PhpError, PhpResult};
use crate::numeric::{self, Number, Numericity};
use crate::{Key, PhpArray, Value, rt};

fn count_recursive(a: &PhpArray) -> usize {
    a.values()
        .map(|v| match v {
            Value::Array(inner) => 1 + count_recursive(inner),
            _ => 1,
        })
        .sum()
}

pub fn count(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = match &args[0] {
        Value::Array(a) => a,
        other => {
            return Err(PhpError::type_error(format!(
                "count(): Argument #1 ($value) must be of type Countable|array, {} given",
                other.type_name()
            )));
        }
    };
    let mode = if args.len() > 1 {
        int_arg("count", args, 1, "mode", ctx)?
    } else {
        0
    };
    Ok(Value::from(if mode == 1 {
        count_recursive(a)
    } else {
        a.len()
    }))
}

pub fn sizeof(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    count(args, ctx)
}

fn matches(needle: &Value, v: &Value, strict: bool) -> bool {
    if strict {
        rt::strict_eq(needle, v)
    } else {
        rt::loose_eq(needle, v)
    }
}

pub fn array_keys(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_keys", args, 0, "array")?;
    let strict = bool_arg("array_keys", args, 2, "strict")?;
    let keys = a
        .iter()
        .filter(|(_, v)| args.get(1).is_none_or(|needle| matches(needle, v, strict)))
        .map(|(k, _)| k.to_value());
    Ok(Value::Array(PhpArray::from_list(keys)))
}

pub fn array_values(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_values", args, 0, "array")?;
    Ok(Value::Array(PhpArray::from_list(a.values().cloned())))
}

pub fn in_array(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("in_array", args, 1, "haystack")?;
    let strict = bool_arg("in_array", args, 2, "strict")?;
    Ok(Value::Bool(
        a.values().any(|v| matches(&args[0], v, strict)),
    ))
}

pub fn array_search(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_search", args, 1, "haystack")?;
    let strict = bool_arg("array_search", args, 2, "strict")?;
    Ok(a.iter()
        .find(|(_, v)| matches(&args[0], v, strict))
        .map_or(Value::Bool(false), |(k, _)| k.to_value()))
}

pub fn array_key_exists(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_key_exists", args, 1, "array")?;
    let key = rt::key(&args[0], ctx)?;
    Ok(Value::Bool(a.contains_key(&key)))
}

pub fn key_exists(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    array_key_exists(args, ctx)
}

pub fn array_merge(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let mut out = PhpArray::new();
    for i in 0..args.len() {
        for (k, v) in array_arg("array_merge", args, i, "arrays")?.iter() {
            match k {
                Key::Int(_) => {
                    out.push(v.clone());
                }
                Key::Str(_) => out.insert(k.clone(), v.clone()),
            }
        }
    }
    Ok(Value::Array(out))
}

pub fn array_reverse(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_reverse", args, 0, "array")?;
    let preserve = bool_arg("array_reverse", args, 1, "preserve_keys")?;
    let entries: Vec<(&Key, &Value)> = a.iter().collect();
    let mut out = PhpArray::new();
    for (k, v) in entries.into_iter().rev() {
        match k {
            Key::Int(_) if !preserve => {
                out.push(v.clone());
            }
            _ => out.insert(k.clone(), v.clone()),
        }
    }
    Ok(Value::Array(out))
}

pub fn array_slice(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_slice", args, 0, "array")?;
    let n = a.len() as i64;
    let offset = int_arg("array_slice", args, 1, "offset", ctx)?;
    let preserve = bool_arg("array_slice", args, 3, "preserve_keys")?;
    let start = if offset < 0 {
        (n + offset).max(0)
    } else {
        offset.min(n)
    };
    let available = n - start;
    let length = match args.get(2) {
        None | Some(Value::Null) => available,
        Some(_) => {
            let l = int_arg("array_slice", args, 2, "length", ctx)?;
            if l < 0 {
                (available + l).max(0)
            } else {
                l.min(available)
            }
        }
    };
    let mut out = PhpArray::new();
    for (k, v) in a.iter().skip(start as usize).take(length as usize) {
        match k {
            Key::Int(_) if !preserve => {
                out.push(v.clone());
            }
            _ => out.insert(k.clone(), v.clone()),
        }
    }
    Ok(Value::Array(out))
}

pub fn range(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let single_char = |v: &Value| match v {
        Value::Str(s) if s.len() == 1 && !s.as_bytes()[0].is_ascii_digit() => Some(s.as_bytes()[0]),
        _ => None,
    };
    if let (Some(a), Some(b)) = (single_char(&args[0]), single_char(&args[1])) {
        let step = if args.len() > 2 {
            int_arg("range", args, 2, "step", ctx)?
                .unsigned_abs()
                .max(1)
        } else {
            1
        };
        let chars: Vec<u8> = if a <= b {
            (a..=b).step_by(step as usize).collect()
        } else {
            (b..=a).rev().step_by(step as usize).collect()
        };
        return Ok(Value::Array(PhpArray::from_list(
            chars.into_iter().map(|c| Value::from(vec![c])),
        )));
    }
    let start = number_arg("range", args, 0, "start", ctx)?;
    let end = number_arg("range", args, 1, "end", ctx)?;
    let step = if args.len() > 2 {
        number_arg("range", args, 2, "step", ctx)?
    } else {
        Number::Int(1)
    };
    if step.as_f64() == 0.0 {
        return Err(PhpError::value_error(
            "range(): Argument #3 ($step) cannot be 0",
        ));
    }
    let mut out = PhpArray::new();
    match (start, end, step) {
        (Number::Int(a), Number::Int(b), Number::Int(s)) => {
            let s = s.unsigned_abs() as i64;
            if a <= b {
                let mut i = a;
                while i <= b {
                    out.push(Value::Int(i));
                    let Some(next) = i.checked_add(s) else { break };
                    i = next;
                }
            } else {
                let mut i = a;
                while i >= b {
                    out.push(Value::Int(i));
                    let Some(next) = i.checked_sub(s) else { break };
                    i = next;
                }
            }
        }
        _ => {
            let (a, b, s) = (start.as_f64(), end.as_f64(), step.as_f64().abs());
            let count = ((b - a).abs() / s).floor() as i64;
            let dir = if a <= b { 1.0 } else { -1.0 };
            for i in 0..=count {
                out.push(Value::Float(a + dir * i as f64 * s));
            }
        }
    }
    Ok(Value::Array(out))
}

type BinaryOp = fn(&Value, &Value, &mut Context) -> PhpResult<Value>;

fn fold_numbers(
    func: &str,
    args: &[Value],
    init: i64,
    op: BinaryOp,
    ctx: &mut Context,
) -> PhpResult<Value> {
    let a = array_arg(func, args, 0, "array")?;
    let mut acc = Value::Int(init);
    for v in a.values() {
        let n = match v {
            Value::Int(_) | Value::Float(_) => v.clone(),
            Value::Str(s) => match numeric::classify(s.as_bytes()) {
                Numericity::Numeric(x) | Numericity::Leading(x) => Value::from(x),
                Numericity::NonNumeric => Value::Int(0),
            },
            Value::Bool(b) => Value::Int(*b as i64),
            Value::Null => Value::Int(0),
            Value::Array(_) => {
                ctx.warn(format!("{func}(): Addition is not supported on type array"));
                continue;
            }
        };
        acc = op(&acc, &n, ctx)?;
    }
    Ok(acc)
}

pub fn array_sum(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    fold_numbers("array_sum", args, 0, rt::add, ctx)
}

pub fn array_product(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    fold_numbers("array_product", args, 1, rt::mul, ctx)
}

pub fn array_flip(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_flip", args, 0, "array")?;
    let mut out = PhpArray::new();
    for (k, v) in a.iter() {
        match v {
            Value::Int(_) | Value::Str(_) => out.insert(rt::key(v, ctx)?, k.to_value()),
            _ => ctx.warn("array_flip(): Can only flip string and integer values, entry skipped"),
        }
    }
    Ok(Value::Array(out))
}

pub fn array_unique(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_unique", args, 0, "array")?;
    let mut seen = std::collections::HashSet::new();
    let mut out = PhpArray::new();
    for (k, v) in a.iter() {
        // Default flag SORT_STRING: values are equal if their string forms are.
        if seen.insert(v.to_php_string()) {
            out.insert(k.clone(), v.clone());
        }
    }
    Ok(Value::Array(out))
}

pub fn array_fill(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let start = int_arg("array_fill", args, 0, "start_index", ctx)?;
    let count = int_arg("array_fill", args, 1, "count", ctx)?;
    if count < 0 {
        return Err(PhpError::value_error(
            "array_fill(): Argument #2 ($count) must be greater than or equal to 0",
        ));
    }
    let mut out = PhpArray::new();
    for i in 0..count {
        out.insert(Key::Int(start + i), args[2].clone());
    }
    Ok(Value::Array(out))
}

pub fn array_combine(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let keys = array_arg("array_combine", args, 0, "keys")?;
    let values = array_arg("array_combine", args, 1, "values")?;
    if keys.len() != values.len() {
        return Err(PhpError::value_error(
            "array_combine(): Argument #1 ($keys) and argument #2 ($values) must have the same number of elements",
        ));
    }
    let mut out = PhpArray::new();
    for (k, v) in keys.values().zip(values.values()) {
        let key = match k {
            Value::Array(_) => rt::key(&rt::cast_string(k, ctx), ctx)?,
            other => rt::key(&Value::Str(other.to_php_string()), ctx)?,
        };
        out.insert(key, v.clone());
    }
    Ok(Value::Array(out))
}

pub fn array_key_first(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_key_first", args, 0, "array")?;
    Ok(a.keys().next().map_or(Value::Null, Key::to_value))
}

pub fn array_key_last(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_arg("array_key_last", args, 0, "array")?;
    Ok(a.keys().last().map_or(Value::Null, Key::to_value))
}

// ----------------------------------------------------------- sorting

fn sort_flags(func: &str, args: &[Value], ctx: &mut Context) -> PhpResult<i64> {
    // `args` here excludes the by-reference array, so flags are args[0].
    if args.is_empty() {
        Ok(0)
    } else {
        int_arg(func, args, 0, "flags", ctx)
    }
}

fn compare_with_flags(a: &Value, b: &Value, flags: i64) -> Ordering {
    match flags & !8 {
        1 => {
            let (x, y) = (a.to_float(), b.to_float());
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
        2 => a
            .to_php_string()
            .as_bytes()
            .cmp(b.to_php_string().as_bytes()),
        _ => rt::compare(a, b),
    }
}

/// What a sort compares.
#[derive(Clone, Copy)]
enum SortBy {
    Value,
    Key,
}

/// The direction of a sort.
#[derive(Clone, Copy)]
enum Order {
    Ascending,
    Descending,
}

/// Whether sorted values keep their keys or are renumbered from 0.
#[derive(Clone, Copy)]
enum Keys {
    Keep,
    Renumber,
}

/// Stable sort (PHP 8 sorts are stable) of the array's entries.
fn sort_entries(
    target: &mut Value,
    func: &str,
    args: &[Value],
    ctx: &mut Context,
    by: SortBy,
    order: Order,
    keys: Keys,
) -> PhpResult<Value> {
    let flags = sort_flags(func, args, ctx)?;
    let a = array_target(func, target, "array")?;
    let mut entries: Vec<(Key, Value)> = a.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.sort_by(|(k1, v1), (k2, v2)| {
        let ord = match by {
            SortBy::Key => compare_with_flags(&k1.to_value(), &k2.to_value(), flags),
            SortBy::Value => compare_with_flags(v1, v2, flags),
        };
        match order {
            Order::Ascending => ord,
            Order::Descending => ord.reverse(),
        }
    });
    let mut out = PhpArray::new();
    for (k, v) in entries {
        match keys {
            Keys::Keep => {
                out.insert(k, v);
            }
            Keys::Renumber => {
                out.push(v);
            }
        }
    }
    *a = out;
    Ok(Value::Bool(true))
}

pub fn sort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "sort",
        args,
        ctx,
        SortBy::Value,
        Order::Ascending,
        Keys::Renumber,
    )
}

pub fn rsort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "rsort",
        args,
        ctx,
        SortBy::Value,
        Order::Descending,
        Keys::Renumber,
    )
}

pub fn asort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "asort",
        args,
        ctx,
        SortBy::Value,
        Order::Ascending,
        Keys::Keep,
    )
}

pub fn arsort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "arsort",
        args,
        ctx,
        SortBy::Value,
        Order::Descending,
        Keys::Keep,
    )
}

pub fn ksort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "ksort",
        args,
        ctx,
        SortBy::Key,
        Order::Ascending,
        Keys::Keep,
    )
}

pub fn krsort(target: &mut Value, args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    sort_entries(
        target,
        "krsort",
        args,
        ctx,
        SortBy::Key,
        Order::Descending,
        Keys::Keep,
    )
}

// ------------------------------------------------- stack and queue ops

pub fn array_push(target: &mut Value, args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_target("array_push", target, "array")?;
    for v in args {
        if !a.push(v.clone()) {
            return Err(PhpError::error(
                "Cannot add element to the array as the next element is already occupied",
            ));
        }
    }
    Ok(Value::from(a.len()))
}

pub fn array_pop(target: &mut Value, _args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_target("array_pop", target, "array")?;
    Ok(a.pop().unwrap_or_default())
}

pub fn array_shift(target: &mut Value, _args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_target("array_shift", target, "array")?;
    let Some((first, _)) = a.get_index(0) else {
        return Ok(Value::Null);
    };
    let first = first.clone();
    let value = a.remove(&first).unwrap_or_default();
    *a = a.renumbered();
    Ok(value)
}

pub fn array_unshift(target: &mut Value, args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    let a = array_target("array_unshift", target, "array")?;
    let mut out = PhpArray::from_list(args.iter().cloned());
    for (k, v) in a.iter() {
        match k {
            Key::Int(_) => {
                out.push(v.clone());
            }
            Key::Str(_) => out.insert(k.clone(), v.clone()),
        }
    }
    *a = out;
    Ok(Value::from(a.len()))
}
