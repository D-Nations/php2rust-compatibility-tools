use std::{iter, str};

use super::math::php_round;
use super::{bool_arg, float_arg, int_arg, str_arg};
use crate::context::Context;
use crate::error::{PhpError, PhpResult};
use crate::{PhpArray, Value, format, rt};

fn bytes(v: Vec<u8>) -> PhpResult<Value> {
    Ok(Value::from(v))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(haystack.len());
    }
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn split<'a>(s: &'a [u8], sep: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = Vec::new();
    let mut rest = s;
    while let Some(i) = find(rest, sep) {
        parts.push(&rest[..i]);
        rest = &rest[i + sep.len()..];
    }
    parts.push(rest);
    parts
}

fn replace_all(subject: &[u8], search: &[u8], replace: &[u8]) -> Vec<u8> {
    if search.is_empty() {
        return subject.to_vec();
    }
    split(subject, search).join(replace)
}

pub fn strlen(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    Ok(Value::from(
        str_arg("strlen", args, 0, "string", ctx)?.len(),
    ))
}

pub fn strtoupper(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    bytes(
        str_arg("strtoupper", args, 0, "string", ctx)?
            .as_bytes()
            .to_ascii_uppercase(),
    )
}

pub fn strtolower(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    bytes(
        str_arg("strtolower", args, 0, "string", ctx)?
            .as_bytes()
            .to_ascii_lowercase(),
    )
}

pub fn ucfirst(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut s = str_arg("ucfirst", args, 0, "string", ctx)?
        .as_bytes()
        .to_vec();
    if let Some(b) = s.first_mut() {
        b.make_ascii_uppercase();
    }
    bytes(s)
}

pub fn lcfirst(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut s = str_arg("lcfirst", args, 0, "string", ctx)?
        .as_bytes()
        .to_vec();
    if let Some(b) = s.first_mut() {
        b.make_ascii_lowercase();
    }
    bytes(s)
}

pub fn ucwords(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut s = str_arg("ucwords", args, 0, "string", ctx)?
        .as_bytes()
        .to_vec();
    let delimiters = match args.get(1) {
        Some(_) => str_arg("ucwords", args, 1, "separators", ctx)?
            .as_bytes()
            .to_vec(),
        None => b" \t\r\n\x0c\x0b".to_vec(),
    };
    let mut at_word_start = true;
    for b in s.iter_mut() {
        if at_word_start {
            b.make_ascii_uppercase();
        }
        at_word_start = delimiters.contains(b);
    }
    bytes(s)
}

pub fn str_repeat(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("str_repeat", args, 0, "string", ctx)?;
    let times = int_arg("str_repeat", args, 1, "times", ctx)?;
    if times < 0 {
        return Err(PhpError::value_error(
            "str_repeat(): Argument #2 ($times) must be greater than or equal to 0",
        ));
    }
    bytes(s.as_bytes().repeat(times as usize))
}

pub fn substr(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("substr", args, 0, "string", ctx)?;
    let len = s.len() as i64;
    let mut offset = int_arg("substr", args, 1, "offset", ctx)?;
    if offset > len {
        return Ok(Value::from(""));
    }
    if offset < 0 {
        offset = (len + offset).max(0);
    }
    let available = len - offset;
    let length = match args.get(2) {
        None | Some(Value::Null) => available,
        Some(_) => {
            let l = int_arg("substr", args, 2, "length", ctx)?;
            if l < 0 {
                (available + l).max(0)
            } else {
                l.min(available)
            }
        }
    };
    bytes(s.as_bytes()[offset as usize..(offset + length) as usize].to_vec())
}

fn search_offset(
    func: &str,
    haystack_len: usize,
    args: &[Value],
    ctx: &mut Context,
) -> PhpResult<usize> {
    let offset = if args.len() > 2 {
        int_arg(func, args, 2, "offset", ctx)?
    } else {
        0
    };
    let len = haystack_len as i64;
    let start = if offset < 0 { len + offset } else { offset };
    if start < 0 || start > len {
        return Err(PhpError::value_error(format!(
            "{func}(): Argument #3 ($offset) must be contained in argument #1 ($haystack)"
        )));
    }
    Ok(start as usize)
}

fn position(found: Option<usize>, base: usize) -> PhpResult<Value> {
    Ok(found.map_or(Value::Bool(false), |i| Value::from(base + i)))
}

pub fn strpos(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("strpos", args, 0, "haystack", ctx)?;
    let n = str_arg("strpos", args, 1, "needle", ctx)?;
    let start = search_offset("strpos", h.len(), args, ctx)?;
    position(find(&h.as_bytes()[start..], n.as_bytes()), start)
}

pub fn stripos(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("stripos", args, 0, "haystack", ctx)?
        .as_bytes()
        .to_ascii_lowercase();
    let n = str_arg("stripos", args, 1, "needle", ctx)?
        .as_bytes()
        .to_ascii_lowercase();
    let start = search_offset("stripos", h.len(), args, ctx)?;
    position(find(&h[start..], &n), start)
}

pub fn strrpos(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("strrpos", args, 0, "haystack", ctx)?;
    let n = str_arg("strrpos", args, 1, "needle", ctx)?;
    let start = search_offset("strrpos", h.len(), args, ctx)?;
    position(rfind(&h.as_bytes()[start..], n.as_bytes()), start)
}

pub fn str_contains(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("str_contains", args, 0, "haystack", ctx)?;
    let n = str_arg("str_contains", args, 1, "needle", ctx)?;
    Ok(Value::Bool(find(h.as_bytes(), n.as_bytes()).is_some()))
}

pub fn str_starts_with(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("str_starts_with", args, 0, "haystack", ctx)?;
    let n = str_arg("str_starts_with", args, 1, "needle", ctx)?;
    Ok(Value::Bool(h.as_bytes().starts_with(n.as_bytes())))
}

pub fn str_ends_with(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let h = str_arg("str_ends_with", args, 0, "haystack", ctx)?;
    let n = str_arg("str_ends_with", args, 1, "needle", ctx)?;
    Ok(Value::Bool(h.as_bytes().ends_with(n.as_bytes())))
}

pub fn str_replace(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = match (&args[0], &args[1]) {
        (Value::Array(search), Value::Array(replace)) => {
            let mut replacements = replace.values();
            search
                .values()
                .map(|s| {
                    let r = replacements
                        .next()
                        .map(|r| rt::to_str(r, ctx))
                        .unwrap_or_default();
                    (
                        rt::to_str(s, ctx).as_bytes().to_vec(),
                        r.as_bytes().to_vec(),
                    )
                })
                .collect()
        }
        (Value::Array(search), replace) => {
            let r = rt::to_str(replace, ctx).as_bytes().to_vec();
            search
                .values()
                .map(|s| (rt::to_str(s, ctx).as_bytes().to_vec(), r.clone()))
                .collect()
        }
        (_, Value::Array(_)) => {
            return Err(PhpError::type_error(
                "str_replace(): Argument #2 ($replace) must be of type string when argument #1 ($search) is a string",
            ));
        }
        (_, replace) => vec![(
            str_arg("str_replace", args, 0, "search", ctx)?
                .as_bytes()
                .to_vec(),
            rt::to_str(replace, ctx).as_bytes().to_vec(),
        )],
    };
    let apply = |subject: &[u8]| {
        pairs
            .iter()
            .fold(subject.to_vec(), |acc, (s, r)| replace_all(&acc, s, r))
    };
    match &args[2] {
        Value::Array(subjects) => {
            let mut out = PhpArray::new();
            for (k, v) in subjects.iter() {
                let replaced = match v {
                    Value::Array(_) => v.clone(),
                    other => Value::from(apply(other.to_php_string().as_bytes())),
                };
                out.insert(k.clone(), replaced);
            }
            Ok(Value::Array(out))
        }
        _ => bytes(apply(
            str_arg("str_replace", args, 2, "subject", ctx)?.as_bytes(),
        )),
    }
}

/// Builds the character mask for `trim()`, including `a..z` ranges.
fn char_mask(list: &[u8]) -> [bool; 256] {
    let mut mask = [false; 256];
    let mut i = 0;
    while i < list.len() {
        if i + 3 < list.len()
            && list[i + 1] == b'.'
            && list[i + 2] == b'.'
            && list[i + 3] >= list[i]
        {
            for c in list[i]..=list[i + 3] {
                mask[c as usize] = true;
            }
            i += 4;
        } else {
            mask[list[i] as usize] = true;
            i += 1;
        }
    }
    mask
}

/// Which ends of a string `trim_impl` strips.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ends {
    Both,
    Start,
    End,
}

fn trim_impl(func: &str, args: &[Value], ends: Ends, ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg(func, args, 0, "string", ctx)?;
    let mask = match args.get(1) {
        Some(_) => char_mask(str_arg(func, args, 1, "characters", ctx)?.as_bytes()),
        None => char_mask(b" \n\r\t\x0b\0"),
    };
    let b = s.as_bytes();
    let mut start = 0;
    let mut end = b.len();
    if ends != Ends::End {
        while start < end && mask[b[start] as usize] {
            start += 1;
        }
    }
    if ends != Ends::Start {
        while end > start && mask[b[end - 1] as usize] {
            end -= 1;
        }
    }
    bytes(b[start..end].to_vec())
}

pub fn trim(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    trim_impl("trim", args, Ends::Both, ctx)
}

pub fn ltrim(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    trim_impl("ltrim", args, Ends::Start, ctx)
}

pub fn rtrim(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    trim_impl("rtrim", args, Ends::End, ctx)
}

pub fn implode(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let (separator, pieces) = match args {
        [Value::Array(a)] => (Vec::new(), a),
        [sep, Value::Array(a)] => (rt::to_str(sep, ctx).as_bytes().to_vec(), a),
        [Value::Array(_), other] | [_, other] => {
            return Err(PhpError::type_error(format!(
                "implode(): Argument #2 ($array) must be of type ?array, {} given",
                other.type_name()
            )));
        }
        [other] => {
            return Err(PhpError::type_error(format!(
                "implode(): Argument #1 ($array) must be of type array, {} given",
                other.type_name()
            )));
        }
        _ => unreachable!("arity is checked by the transpiler"),
    };
    let mut out = Vec::new();
    for (i, v) in pieces.values().enumerate() {
        if i > 0 {
            out.extend_from_slice(&separator);
        }
        out.extend_from_slice(rt::to_str(v, ctx).as_bytes());
    }
    bytes(out)
}

pub fn join(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    implode(args, ctx)
}

pub fn explode(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let sep = str_arg("explode", args, 0, "separator", ctx)?;
    let s = str_arg("explode", args, 1, "string", ctx)?;
    if sep.is_empty() {
        return Err(PhpError::value_error(
            "explode(): Argument #1 ($separator) cannot be empty",
        ));
    }
    let limit = if args.len() > 2 {
        int_arg("explode", args, 2, "limit", ctx)?
    } else {
        i64::MAX
    };
    let parts = split(s.as_bytes(), sep.as_bytes());
    let parts: Vec<Vec<u8>> = if limit > 0 {
        let limit = limit.min(parts.len() as i64) as usize;
        let mut out: Vec<Vec<u8>> = parts[..limit - 1].iter().map(|p| p.to_vec()).collect();
        out.push(parts[limit - 1..].join(sep.as_bytes()));
        out
    } else if limit < 0 {
        let keep = parts.len().saturating_sub(limit.unsigned_abs() as usize);
        parts[..keep].iter().map(|p| p.to_vec()).collect()
    } else {
        vec![s.as_bytes().to_vec()]
    };
    Ok(Value::Array(PhpArray::from_list(
        parts.into_iter().map(Value::from),
    )))
}

pub fn strrev(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let mut s = str_arg("strrev", args, 0, "string", ctx)?
        .as_bytes()
        .to_vec();
    s.reverse();
    bytes(s)
}

pub fn str_pad(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("str_pad", args, 0, "string", ctx)?;
    let length = int_arg("str_pad", args, 1, "length", ctx)?;
    let pad = match args.get(2) {
        Some(_) => str_arg("str_pad", args, 2, "pad_string", ctx)?
            .as_bytes()
            .to_vec(),
        None => b" ".to_vec(),
    };
    let pad_type = if args.len() > 3 {
        int_arg("str_pad", args, 3, "pad_type", ctx)?
    } else {
        1
    };
    if length <= s.len() as i64 {
        return Ok(Value::Str(s));
    }
    if pad.is_empty() {
        return Err(PhpError::value_error(
            "str_pad(): Argument #3 ($pad_string) must be a non-empty string",
        ));
    }
    let total = length as usize - s.len();
    let left = match pad_type {
        0 => total,
        1 => 0,
        2 => total / 2,
        _ => {
            return Err(PhpError::value_error(
                "str_pad(): Argument #4 ($pad_type) must be STR_PAD_LEFT, STR_PAD_RIGHT, or STR_PAD_BOTH",
            ));
        }
    };
    let padding = |n: usize| (0..n).map(|i| pad[i % pad.len()]).collect::<Vec<u8>>();
    let mut out = padding(left);
    out.extend_from_slice(s.as_bytes());
    out.extend(padding(total - left));
    bytes(out)
}

pub fn str_split(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("str_split", args, 0, "string", ctx)?;
    let length = if args.len() > 1 {
        int_arg("str_split", args, 1, "length", ctx)?
    } else {
        1
    };
    if length < 1 {
        return Err(PhpError::value_error(
            "str_split(): Argument #2 ($length) must be greater than 0",
        ));
    }
    let chunks = s
        .as_bytes()
        .chunks(length as usize)
        .map(|c| Value::from(c.to_vec()));
    Ok(Value::Array(PhpArray::from_list(chunks)))
}

/// Whether `s` (just after a `&`) starts with an entity such as `amp;`,
/// `#039;` or `#x27;`, for `htmlspecialchars(..., double_encode: false)`.
fn starts_with_entity(s: &[u8]) -> bool {
    let end = match s.iter().position(|&b| b == b';') {
        Some(end) if end > 0 => end,
        _ => return false,
    };
    let body = &s[..end];
    match body {
        [b'#', b'x' | b'X', hex @ ..] => !hex.is_empty() && hex.iter().all(u8::is_ascii_hexdigit),
        [b'#', dec @ ..] => !dec.is_empty() && dec.iter().all(u8::is_ascii_digit),
        [first, rest @ ..] => {
            first.is_ascii_alphabetic() && rest.iter().all(u8::is_ascii_alphanumeric)
        }
        [] => false,
    }
}

pub fn htmlspecialchars(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    const ENT_HTML_QUOTE_SINGLE: i64 = 1;
    const ENT_HTML_QUOTE_DOUBLE: i64 = 2;
    const ENT_IGNORE: i64 = 4;
    const ENT_SUBSTITUTE: i64 = 8;
    const DOCTYPE_MASK: i64 = 16 | 32;
    let s = str_arg("htmlspecialchars", args, 0, "string", ctx)?;
    let flags = if args.len() > 1 {
        int_arg("htmlspecialchars", args, 1, "flags", ctx)?
    } else {
        3 | 8
    };
    let double_encode = args.len() <= 3 || bool_arg("htmlspecialchars", args, 3, "double_encode")?;
    let input: Vec<u8> = match str::from_utf8(s.as_bytes()) {
        Ok(_) => s.as_bytes().to_vec(),
        Err(_) if flags & ENT_SUBSTITUTE != 0 => String::from_utf8_lossy(s.as_bytes())
            .into_owned()
            .into_bytes(),
        Err(_) if flags & ENT_IGNORE != 0 => String::from_utf8_lossy(s.as_bytes())
            .replace('\u{FFFD}', "")
            .into_bytes(),
        Err(_) => return Ok(Value::from("")),
    };
    let mut out = Vec::with_capacity(input.len());
    for (i, &b) in input.iter().enumerate() {
        match b {
            b'&' if !double_encode && starts_with_entity(&input[i + 1..]) => out.push(b'&'),
            b'&' => out.extend_from_slice(b"&amp;"),
            b'<' => out.extend_from_slice(b"&lt;"),
            b'>' => out.extend_from_slice(b"&gt;"),
            b'"' if flags & ENT_HTML_QUOTE_DOUBLE != 0 => out.extend_from_slice(b"&quot;"),
            b'\'' if flags & ENT_HTML_QUOTE_SINGLE != 0 => {
                // HTML 4.01 has no &apos;; XML1, XHTML and HTML5 do.
                out.extend_from_slice(if flags & DOCTYPE_MASK == 0 {
                    b"&#039;"
                } else {
                    b"&apos;"
                })
            }
            _ => out.push(b),
        }
    }
    bytes(out)
}

pub fn nl2br(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("nl2br", args, 0, "string", ctx)?;
    let xhtml = args.len() < 2 || bool_arg("nl2br", args, 1, "use_xhtml")?;
    let br: &[u8] = if xhtml { b"<br />" } else { b"<br>" };
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' || b[i] == b'\n' {
            out.extend_from_slice(br);
            out.push(b[i]);
            // "\r\n" and "\n\r" are single line breaks.
            if i + 1 < b.len() && (b[i + 1] == b'\r' || b[i + 1] == b'\n') && b[i + 1] != b[i] {
                out.push(b[i + 1]);
                i += 1;
            }
        } else {
            out.push(b[i]);
        }
        i += 1;
    }
    bytes(out)
}

pub fn ord(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let s = str_arg("ord", args, 0, "character", ctx)?;
    Ok(Value::Int(s.as_bytes().first().copied().unwrap_or(0) as i64))
}

pub fn chr(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let code = int_arg("chr", args, 0, "codepoint", ctx)?;
    bytes(vec![code.rem_euclid(256) as u8])
}

pub fn strcmp(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let a = str_arg("strcmp", args, 0, "string1", ctx)?;
    let b = str_arg("strcmp", args, 1, "string2", ctx)?;
    Ok(Value::Int(a.as_bytes().cmp(b.as_bytes()) as i64))
}

pub fn number_format(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let num = float_arg("number_format", args, 0, "num", ctx)?;
    let decimals = if args.len() > 1 {
        int_arg("number_format", args, 1, "decimals", ctx)?.max(0)
    } else {
        0
    };
    let dec_point = match args.get(2) {
        Some(_) => str_arg("number_format", args, 2, "decimal_separator", ctx)?
            .as_bytes()
            .to_vec(),
        None => b".".to_vec(),
    };
    let thousands = match args.get(3) {
        Some(_) => str_arg("number_format", args, 3, "thousands_separator", ctx)?
            .as_bytes()
            .to_vec(),
        None => b",".to_vec(),
    };
    let rounded = php_round(num, decimals);
    let text = format!("{:.*}", decimals as usize, rounded.abs());
    let (int_part, frac_part) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text.as_str(), None),
    };
    let mut out = Vec::new();
    if rounded < 0.0 && text.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        out.push(b'-');
    }
    for (i, d) in int_part.bytes().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            out.extend_from_slice(&thousands);
        }
        out.push(d);
    }
    if let Some(frac) = frac_part {
        out.extend_from_slice(&dec_point);
        out.extend_from_slice(frac.as_bytes());
    }
    bytes(out)
}

pub fn sprintf(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let format = str_arg("sprintf", args, 0, "format", ctx)?;
    bytes(format_printf(format.as_bytes(), &args[1..], ctx)?)
}

pub fn printf(args: &[Value], ctx: &mut Context) -> PhpResult<Value> {
    let format = str_arg("printf", args, 0, "format", ctx)?;
    let out = format_printf(format.as_bytes(), &args[1..], ctx)?;
    ctx.write(&out);
    Ok(Value::from(out.len()))
}

/// Which side of the field a `printf` conversion sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Right,
}

/// What a `printf` conversion produced. A number keeps its sign in front of
/// zero padding (`-0042`); text does not.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    Number,
    Text,
}

fn pad_to(body: Vec<u8>, width: usize, pad: u8, align: Align, content: Content) -> Vec<u8> {
    if body.len() >= width {
        return body;
    }
    let fill = width - body.len();
    if align == Align::Left {
        let mut out = body;
        out.extend(iter::repeat_n(pad, fill));
        return out;
    }
    let mut out = Vec::with_capacity(width);
    let mut body = body.as_slice();
    // Zero padding goes between the sign and the digits: "-0042".
    if content == Content::Number && pad == b'0' && matches!(body.first(), Some(b'-' | b'+')) {
        out.push(body[0]);
        body = &body[1..];
    }
    out.extend(iter::repeat_n(pad, fill));
    out.extend_from_slice(body);
    out
}

/// PHP's `%e`: the exponent has no zero padding (`1.5e+3`).
fn php_exponential(f: f64, precision: usize, upper: bool) -> String {
    let s = format!("{:.*e}", precision, f);
    let (mantissa, exponent) = s.split_once('e').expect("exponent format");
    let exponent: i32 = exponent.parse().expect("exponent");
    let e = if upper { 'E' } else { 'e' };
    format!(
        "{mantissa}{e}{}{}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    )
}

fn format_printf(format: &[u8], args: &[Value], ctx: &mut Context) -> PhpResult<Vec<u8>> {
    let mut out = Vec::new();
    let mut next_arg = 0usize;
    let mut i = 0;
    let at = |i: usize| format.get(i).copied();
    while i < format.len() {
        if format[i] != b'%' {
            out.push(format[i]);
            i += 1;
            continue;
        }
        i += 1;
        if at(i) == Some(b'%') {
            out.push(b'%');
            i += 1;
            continue;
        }
        // %[argnum$][flags][width][.precision]specifier
        let mut argnum = None;
        let digits_end = (i..format.len())
            .find(|&j| !format[j].is_ascii_digit())
            .unwrap_or(format.len());
        if digits_end > i && at(digits_end) == Some(b'$') {
            let n: usize = str::from_utf8(&format[i..digits_end])
                .expect("ASCII digits are UTF-8")
                .parse()
                .unwrap_or(0);
            if n == 0 {
                return Err(PhpError::value_error(
                    "Argument number specifier must be greater than zero and less than 2147483647",
                ));
            }
            argnum = Some(n - 1);
            i = digits_end + 1;
        }
        let (mut align, mut plus, mut pad) = (Align::Right, false, b' ');
        loop {
            match at(i) {
                Some(b'-') => align = Align::Left,
                Some(b'+') => plus = true,
                Some(b'0') => pad = b'0',
                Some(b' ') => pad = b' ',
                Some(b'\'') => {
                    i += 1;
                    pad = at(i).unwrap_or(b' ');
                }
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        while let Some(d @ b'0'..=b'9') = at(i) {
            width = width * 10 + (d - b'0') as usize;
            i += 1;
        }
        let mut precision = None;
        if at(i) == Some(b'.') {
            i += 1;
            let mut p = 0usize;
            while let Some(d @ b'0'..=b'9') = at(i) {
                p = p * 10 + (d - b'0') as usize;
                i += 1;
            }
            precision = Some(p);
        }
        let Some(spec) = at(i) else {
            return Err(PhpError::value_error(
                "Missing format specifier at end of string",
            ));
        };
        i += 1;
        let index = argnum.unwrap_or_else(|| {
            next_arg += 1;
            next_arg - 1
        });
        let Some(arg) = args.get(index) else {
            return Err(PhpError::argument_count_error(format!(
                "{} arguments are required, {} given",
                index + 2,
                args.len() + 1
            )));
        };
        let signed = |s: String, non_negative: bool| -> Vec<u8> {
            if plus && non_negative {
                format!("+{s}").into_bytes()
            } else {
                s.into_bytes()
            }
        };
        let (body, content) = match spec {
            b's' => {
                let mut s = rt::to_str(arg, ctx).as_bytes().to_vec();
                if let Some(p) = precision {
                    s.truncate(p);
                }
                (s, Content::Text)
            }
            b'd' => {
                let n = arg.to_int();
                (signed(n.to_string(), n >= 0), Content::Number)
            }
            b'u' => (
                (arg.to_int() as u64).to_string().into_bytes(),
                Content::Number,
            ),
            b'f' | b'F' => {
                let f = arg.to_float();
                (
                    signed(format!("{:.*}", precision.unwrap_or(6), f), f >= 0.0),
                    Content::Number,
                )
            }
            b'e' | b'E' => {
                let f = arg.to_float();
                (
                    signed(
                        php_exponential(f, precision.unwrap_or(6), spec == b'E'),
                        f >= 0.0,
                    ),
                    Content::Number,
                )
            }
            b'g' | b'G' => {
                let f = arg.to_float();
                let p = match precision {
                    Some(0) => 1,
                    Some(p) => p,
                    None => 6,
                };
                let mut s = format::php_gcvt(f, p);
                if spec == b'g' {
                    s = s.replace('E', "e");
                }
                (signed(s, f >= 0.0), Content::Number)
            }
            b'x' => (
                format!("{:x}", arg.to_int() as u64).into_bytes(),
                Content::Text,
            ),
            b'X' => (
                format!("{:X}", arg.to_int() as u64).into_bytes(),
                Content::Text,
            ),
            b'o' => (
                format!("{:o}", arg.to_int() as u64).into_bytes(),
                Content::Text,
            ),
            b'b' => (
                format!("{:b}", arg.to_int() as u64).into_bytes(),
                Content::Text,
            ),
            b'c' => {
                out.push(arg.to_int() as u8);
                continue;
            }
            other => {
                return Err(PhpError::value_error(format!(
                    "Unknown format specifier \"{}\"",
                    other as char
                )));
            }
        };
        out.extend(pad_to(body, width, pad, align, content));
    }
    Ok(out)
}
