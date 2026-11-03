//! Byte-exact reproductions of PHP's float formatting, `var_dump` and `print_r`.

use crate::array::Key;
use crate::value::Value;

/// `echo $float` / `(string)$float`: PHP's `precision` ini setting (14).
pub fn float_to_string(f: f64) -> String {
    php_gcvt(f, 14)
}

/// `var_dump`, `var_export`, `json_encode`: `serialize_precision = -1`,
/// meaning the shortest string that round-trips.
pub fn float_repr(f: f64) -> String {
    php_gcvt(f, 0)
}

/// Reproduces the output of PHP's `php_gcvt`, built on Rust's own float
/// formatting rather than translated from PHP's source. `precision == 0`
/// selects the shortest round-trip digits with a 17-digit threshold.
pub fn php_gcvt(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "NAN".into();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-INF".into()
        } else {
            "INF".into()
        };
    }
    let ndigit = if precision == 0 { 17 } else { precision } as i32;
    let sci = if precision == 0 {
        format!("{:e}", value.abs())
    } else {
        format!("{:.*e}", precision - 1, value.abs())
    };
    let (mantissa, exponent) = sci.split_once('e').expect("exponent format");
    let exponent: i32 = exponent.parse().expect("exponent");
    let mut digits: Vec<u8> = mantissa.bytes().filter(|b| *b != b'.').collect();
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    let decpt = if value == 0.0 { 1 } else { exponent + 1 };

    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    let digits = String::from_utf8(digits).expect("ascii digits");
    if decpt < -3 || decpt > ndigit {
        let e = decpt - 1;
        out.push_str(&digits[..1]);
        out.push('.');
        out.push_str(if digits.len() == 1 { "0" } else { &digits[1..] });
        out.push('E');
        out.push(if e < 0 { '-' } else { '+' });
        out.push_str(&e.abs().to_string());
    } else if decpt <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-decpt) as usize));
        out.push_str(&digits);
    } else {
        let int_len = decpt as usize;
        if digits.len() <= int_len {
            out.push_str(&digits);
            out.extend(std::iter::repeat_n('0', int_len - digits.len()));
        } else {
            out.push_str(&digits[..int_len]);
            out.push('.');
            out.push_str(&digits[int_len..]);
        }
    }
    out
}

pub fn var_dump(value: &Value, indent: usize, out: &mut Vec<u8>) {
    out.extend(std::iter::repeat_n(b' ', indent));
    match value {
        Value::Null => out.extend_from_slice(b"NULL\n"),
        Value::Bool(b) => out.extend_from_slice(if *b {
            b"bool(true)\n"
        } else {
            b"bool(false)\n"
        }),
        Value::Int(i) => out.extend_from_slice(format!("int({i})\n").as_bytes()),
        Value::Float(f) => out.extend_from_slice(format!("float({})\n", float_repr(*f)).as_bytes()),
        Value::Str(s) => {
            out.extend_from_slice(format!("string({}) \"", s.len()).as_bytes());
            out.extend_from_slice(s.as_bytes());
            out.extend_from_slice(b"\"\n");
        }
        Value::Array(a) => {
            out.extend_from_slice(format!("array({}) {{\n", a.len()).as_bytes());
            for (k, v) in a.iter() {
                out.extend(std::iter::repeat_n(b' ', indent + 2));
                match k {
                    Key::Int(i) => out.extend_from_slice(format!("[{i}]=>\n").as_bytes()),
                    Key::Str(s) => {
                        out.extend_from_slice(b"[\"");
                        out.extend_from_slice(s.as_bytes());
                        out.extend_from_slice(b"\"]=>\n");
                    }
                }
                var_dump(v, indent + 2, out);
            }
            out.extend(std::iter::repeat_n(b' ', indent));
            out.extend_from_slice(b"}\n");
        }
    }
}

pub fn print_r(value: &Value, indent: usize, out: &mut Vec<u8>) {
    match value {
        Value::Array(a) => {
            out.extend_from_slice(b"Array\n");
            out.extend(std::iter::repeat_n(b' ', indent));
            out.extend_from_slice(b"(\n");
            for (k, v) in a.iter() {
                out.extend(std::iter::repeat_n(b' ', indent + 4));
                out.push(b'[');
                match k {
                    Key::Int(i) => out.extend_from_slice(i.to_string().as_bytes()),
                    Key::Str(s) => out.extend_from_slice(s.as_bytes()),
                }
                out.extend_from_slice(b"] => ");
                print_r(v, indent + 8, out);
                out.push(b'\n');
            }
            out.extend(std::iter::repeat_n(b' ', indent));
            out.extend_from_slice(b")\n");
        }
        scalar => out.extend_from_slice(&scalar.to_string_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// `echo $float`: 14 significant digits.
    #[rstest]
    #[case::sum(0.1 + 0.2, "0.3")]
    #[case::third(1.0 / 3.0, "0.33333333333333")]
    #[case::large_exponent(1e20, "1.0E+20")]
    #[case::exponent_threshold(1e14, "1.0E+14")]
    #[case::below_threshold(1e13, "10000000000000")]
    #[case::whole(1.0, "1")]
    #[case::small_negative(-0.0001, "-0.0001")]
    #[case::small_exponent(0.00001, "1.0E-5")]
    #[case::negative_zero(-0.0, "-0")]
    #[case::i64_max(i64::MAX as f64, "9.2233720368548E+18")]
    #[case::infinity(f64::INFINITY, "INF")]
    fn echo_precision(#[case] value: f64, #[case] expected: &str) {
        assert_eq!(float_to_string(value), expected);
    }

    /// `var_dump` and friends: the shortest string that round-trips.
    #[rstest]
    #[case::sum(0.1 + 0.2, "0.30000000000000004")]
    #[case::whole(1.0, "1")]
    #[case::fraction(1.5, "1.5")]
    // Shortest round-trip digits, so 16 digits here, not 17.
    #[case::i64_max(i64::MAX as f64, "9.223372036854776E+18")]
    fn serialize_precision(#[case] value: f64, #[case] expected: &str) {
        assert_eq!(float_repr(value), expected);
    }
}
