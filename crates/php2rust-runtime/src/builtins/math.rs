use std::cmp::Ordering;

use super::{array_arg, float_arg, int_arg, number_arg};
use crate::Value;
use crate::context::Context;
use crate::error::{PhpError, PhpResult};
use crate::numeric::Number;
use crate::rt;

/// PHP's `round()`: pre-rounds to 15 significant digits so that values like
/// 1.005 (stored as 1.00499999...) round the way a person expects, then
/// rounds half away from zero.
pub(crate) fn php_round(value: f64, places: i64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let places = places.clamp(-400, 400) as i32;
    let sci = format!("{:.14e}", value.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("exponent format");
    let exponent: i32 = exponent.parse().expect("exponent");
    let digits: Vec<u64> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| u64::from(b - b'0'))
        .collect();
    let keep = exponent + 1 + places;
    if keep < 0 {
        return 0.0_f64.copysign(value);
    }
    if keep as usize >= digits.len() {
        return value;
    }
    let keep = keep as usize;
    let kept = digits[..keep].iter().fold(0u64, |acc, d| acc * 10 + d);
    let rounded = kept + u64::from(digits[keep] >= 5);
    let scale = exponent + 1 - keep as i32;
    let result: f64 = format!("{rounded}e{scale}").parse().expect("float");
    result.copysign(value)
}

fn number_value(n: Number) -> Value {
    Value::from(n)
}

pub fn abs(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(match number_arg("abs", args, 0, "num", ctx)? {
        Number::Int(i) => i
            .checked_abs()
            .map_or(Value::Float(-(i as f64)), Value::Int),
        Number::Float(f) => Value::Float(f.abs()),
    })
}

fn extremum(func: &str, args: &[Value], want: Ordering) -> PhpResult<Value> {
    let values: Vec<&Value> = if args.len() == 1 {
        let a = array_arg(func, args, 0, "value")?;
        if a.is_empty() {
            return Err(PhpError::value_error(format!(
                "{func}(): Argument #1 ($value) must contain at least one element"
            )));
        }
        a.values().collect()
    } else {
        args.iter().collect()
    };
    let mut best = values[0];
    for v in &values[1..] {
        if rt::compare(v, best) == want {
            best = v;
        }
    }
    Ok(best.clone())
}

pub fn max(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    extremum("max", args, Ordering::Greater)
}

pub fn min(args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    extremum("min", args, Ordering::Less)
}

pub fn floor(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Float(
        number_arg("floor", args, 0, "num", ctx)?.as_f64().floor(),
    ))
}

pub fn ceil(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Float(
        number_arg("ceil", args, 0, "num", ctx)?.as_f64().ceil(),
    ))
}

pub fn round(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let num = number_arg("round", args, 0, "num", ctx)?;
    let places = if args.len() > 1 {
        int_arg("round", args, 1, "precision", ctx)?
    } else {
        0
    };
    Ok(Value::Float(php_round(num.as_f64(), places)))
}

pub fn intdiv(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = int_arg("intdiv", args, 0, "num1", ctx)?;
    let b = int_arg("intdiv", args, 1, "num2", ctx)?;
    if b == 0 {
        return Err(PhpError::division_by_zero("Division by zero"));
    }
    a.checked_div(b).map(Value::Int).ok_or_else(|| {
        PhpError::arithmetic_error("Division of PHP_INT_MIN by -1 is not an integer")
    })
}

pub fn fmod(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let x = float_arg("fmod", args, 0, "num1", ctx)?;
    let y = float_arg("fmod", args, 1, "num2", ctx)?;
    Ok(Value::Float(x % y))
}

pub fn sqrt(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Float(float_arg("sqrt", args, 0, "num", ctx)?.sqrt()))
}

pub fn pow(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let base = number_value(number_arg("pow", args, 0, "num", ctx)?);
    let exp = number_value(number_arg("pow", args, 1, "exponent", ctx)?);
    rt::pow(&base, &exp, ctx)
}

pub fn pi(_args: &[Value], _ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::Float(std::f64::consts::PI))
}

#[cfg(test)]
mod tests {
    use super::php_round;

    #[test]
    fn rounds_like_php() {
        assert_eq!(php_round(1.23456, 2), 1.23);
        assert_eq!(php_round(2.5, 0), 3.0);
        assert_eq!(php_round(-2.5, 0), -3.0);
        assert_eq!(php_round(1.005, 2), 1.01);
        assert_eq!(php_round(0.5, 0), 1.0);
        assert_eq!(php_round(0.4, 0), 0.0);
        assert_eq!(php_round(9.99, 1), 10.0);
        assert_eq!(php_round(1234.0, -2), 1200.0);
        assert_eq!(php_round(1234567.891, 2), 1234567.89);
    }
}
