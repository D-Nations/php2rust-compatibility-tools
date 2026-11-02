//! PHP 8 numeric-string rules (the "Saner numeric strings" RFC).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn as_f64(self) -> f64 {
        match self {
            Number::Int(i) => i as f64,
            Number::Float(f) => f,
        }
    }
}

/// How much of a string is numeric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Numericity {
    /// The whole string is numeric, allowing leading and trailing whitespace: `" 12 "`.
    Numeric(Number),
    /// Only a prefix is numeric: `"12abc"`. Arithmetic uses the prefix and warns.
    Leading(Number),
    /// No numeric prefix: `"abc"`.
    NonNumeric,
}

pub fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

pub fn classify(s: &[u8]) -> Numericity {
    match parse_prefix(s) {
        None => Numericity::NonNumeric,
        Some((n, end)) if s[end..].iter().all(|&b| is_ws(b)) => Numericity::Numeric(n),
        Some((n, _)) => Numericity::Leading(n),
    }
}

/// The numeric value of a fully numeric string, if it is one.
pub fn numeric(s: &[u8]) -> Option<Number> {
    match classify(s) {
        Numericity::Numeric(n) => Some(n),
        _ => None,
    }
}

/// Parses the longest numeric prefix (after leading whitespace), returning the
/// number and the index just past it. Decimal only: PHP does not treat `"0x1A"`
/// as numeric.
fn parse_prefix(s: &[u8]) -> Option<(Number, usize)> {
    let mut i = 0;
    while i < s.len() && is_ws(s[i]) {
        i += 1;
    }
    let start = i;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    let mut is_float = false;
    if i < s.len() && s[i] == b'.' {
        let frac_start = i + 1;
        let mut j = frac_start;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if digits > 0 || j > frac_start {
            digits += j - frac_start;
            is_float = true;
            i = j;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let mut j = i + 1;
        if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
            j += 1;
        }
        if j < s.len() && s[j].is_ascii_digit() {
            while j < s.len() && s[j].is_ascii_digit() {
                j += 1;
            }
            is_float = true;
            i = j;
        }
    }
    // The prefix is pure ASCII by construction.
    let text = std::str::from_utf8(&s[start..i]).ok()?;
    if !is_float {
        if let Ok(n) = text.parse::<i64>() {
            return Some((Number::Int(n), i));
        }
        // Integer overflow: PHP falls back to float.
    }
    let f: f64 = text.parse().ok()?;
    Some((Number::Float(f), i))
}

/// PHP's `zend_dval_to_lval`: NaN and infinities become 0, out-of-range values
/// wrap modulo 2^64 (the 64-bit platform behavior).
pub fn float_to_int(f: f64) -> i64 {
    if !f.is_finite() {
        return 0;
    }
    // i64::MIN and i64::MAX convert to exactly -2^63 and 2^63.
    if (i64::MIN as f64..i64::MAX as f64).contains(&f) {
        return f as i64;
    }
    let two64 = 18_446_744_073_709_551_616.0_f64;
    let wrapped = f.trunc().rem_euclid(two64);
    (wrapped as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_like_php8() {
        assert_eq!(classify(b"123"), Numericity::Numeric(Number::Int(123)));
        assert_eq!(classify(b"  12  "), Numericity::Numeric(Number::Int(12)));
        assert_eq!(classify(b"1e3"), Numericity::Numeric(Number::Float(1000.0)));
        assert_eq!(classify(b".5"), Numericity::Numeric(Number::Float(0.5)));
        assert_eq!(classify(b"12abc"), Numericity::Leading(Number::Int(12)));
        assert_eq!(classify(b"1e"), Numericity::Leading(Number::Int(1)));
        assert_eq!(classify(b"abc"), Numericity::NonNumeric);
        assert_eq!(classify(b""), Numericity::NonNumeric);
        assert_eq!(classify(b"."), Numericity::NonNumeric);
        assert_eq!(classify(b"0x1A"), Numericity::Leading(Number::Int(0)));
        assert_eq!(
            classify(b"9223372036854775808"),
            Numericity::Numeric(Number::Float(i64::MAX as f64))
        );
    }

    #[test]
    fn float_to_int_wraps() {
        assert_eq!(float_to_int(f64::NAN), 0);
        assert_eq!(float_to_int(-3.9), -3);
        assert_eq!(float_to_int(1.8446744073709552e19), 0);
    }
}
