//! PHP's `iconv($from, $to, $string)` on glibc (the oracle's iconv), for a
//! set of charsets: UTF-8, UTF-16, UTF-16BE, UTF-16LE, UTF-32, UTF-32BE,
//! UTF-32LE, UCS-2, ASCII, ISO-8859-1, ISO-8859-15, Windows-1252 and
//! ISO-2022-JP, under their usual names. Any other charset, the
//! `//TRANSLIT` and `//IGNORE` suffixes, and UTF-8 to UTF-8 of the old
//! forms beyond U+10FFFF (which glibc copies) are refused
//! ([`Error::Unsupported`]). ISO-2022-JP is ASCII, JIS-Roman and JIS X
//! 0208 with glibc's table: JIS0208.TXT's characters, no NEC or IBM rows,
//! no half-width katakana; an escape sequence it does not know is read as
//! the character ESC.
//!
//! Written from PHP 8.4.27's observed answers (`tests/data/iconv.json`,
//! `oracle/recorders/php_iconv.php`, `scripts/record.sh iconv`). No PHP or
//! glibc source code is used. The rules seen there: conversion stops at
//! the first character that cannot be read or written, and PHP returns
//! `false` with an `E_NOTICE` ("Detected an illegal character in input
//! string", or "Detected an incomplete multibyte character in input
//! string" when the input ends inside a character). `UTF-16` and `UTF-32`
//! write a byte order mark and little-endian units (not for an empty
//! result), and read a mark if there is one, else little-endian. `UCS-2`
//! is little-endian with no mark, and keeps a U+FEFF it reads.
//!
//! Upstream: none

use std::fmt;

/// Why `iconv()` returned `false`, or why this crate refuses the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A character that cannot be read in `from` or written in `to`.
    Illegal,
    /// The input ends inside a character.
    Incomplete,
    /// A charset (as written) this crate does not convert.
    Unsupported(String),
}

impl Error {
    /// The notice PHP raises with the `false` result (not for
    /// [`Error::Unsupported`], which PHP may well convert).
    #[must_use]
    pub fn notice(&self) -> Option<&'static str> {
        match self {
            Self::Illegal => Some("iconv(): Detected an illegal character in input string"),
            Self::Incomplete => {
                Some("iconv(): Detected an incomplete multibyte character in input string")
            }
            Self::Unsupported(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(name) => {
                write!(f, "iconv() with the charset \"{name}\" is not supported")
            }
            other => f.write_str(other.notice().unwrap_or_default()),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Big,
    Little,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Charset {
    Utf8,
    Ascii,
    Latin1,
    Latin9,
    Cp1252,
    /// UTF-16; `None`: with a byte order mark.
    Utf16(Option<Endian>),
    Utf32(Option<Endian>),
    Ucs2,
    Iso2022Jp,
}

fn charset(name: &str) -> Result<Charset, Error> {
    use Charset as C;
    Ok(match name.to_ascii_uppercase().as_str() {
        "UTF-8" | "UTF8" => C::Utf8,
        "ASCII" | "US-ASCII" | "ANSI_X3.4-1968" => C::Ascii,
        "ISO-8859-1" | "ISO8859-1" | "ISO88591" | "ISO_8859-1" | "LATIN1" | "L1" => C::Latin1,
        "ISO-8859-15" | "ISO8859-15" | "ISO885915" | "ISO_8859-15" | "LATIN-9" | "LATIN9" => {
            C::Latin9
        }
        "WINDOWS-1252" | "CP1252" => C::Cp1252,
        "UTF-16" | "UTF16" => C::Utf16(None),
        "UTF-16BE" => C::Utf16(Some(Endian::Big)),
        "UTF-16LE" => C::Utf16(Some(Endian::Little)),
        "UTF-32" | "UTF32" => C::Utf32(None),
        "UTF-32BE" => C::Utf32(Some(Endian::Big)),
        "UTF-32LE" => C::Utf32(Some(Endian::Little)),
        "UCS-2" | "UCS2" => C::Ucs2,
        "ISO-2022-JP" | "CSISO2022JP" => C::Iso2022Jp,
        _ => return Err(Error::Unsupported(name.to_owned())),
    })
}

/// `iconv($from, $to, $string)`: the converted bytes, or why PHP returned
/// `false` (or why the call is refused).
pub fn iconv(from: &str, to: &str, input: &[u8]) -> Result<Vec<u8>, Error> {
    let from_charset = charset(from)?;
    let to_charset = charset(to)?;
    if from_charset == Charset::Utf8
        && to_charset == Charset::Utf8
        && let Err(e) = std::str::from_utf8(input)
        && matches!(input.get(e.valid_up_to()), Some(0xF4..=0xFD))
    {
        // glibc copies the old 4- to 6-byte forms beyond U+10FFFF from
        // UTF-8 to UTF-8 (and refuses them for any other charset); not
        // reproduced
        return Err(Error::Unsupported(format!(
            "{from} to {to} beyond U+10FFFF"
        )));
    }
    let (from, to) = (from_charset, to_charset);
    let (chars, read_error) = decode(from, input);
    let out = encode(to, &chars)?;
    match read_error {
        Some(error) => Err(error),
        None => Ok(out),
    }
}

/// The characters up to the first one that cannot be read, and the error
/// there.
fn decode(charset: Charset, input: &[u8]) -> (Vec<char>, Option<Error>) {
    match charset {
        Charset::Utf8 => decode_utf8(input),
        Charset::Ascii => single_byte(input, |b| (b < 0x80).then(|| char::from(b))),
        Charset::Latin1 => single_byte(input, |b| Some(char::from(b))),
        Charset::Latin9 => single_byte(input, |b| Some(latin9(b))),
        Charset::Cp1252 => single_byte(input, cp1252),
        Charset::Utf16(endian) => decode_utf16(input, endian, true),
        Charset::Ucs2 => decode_utf16(input, Some(Endian::Little), false),
        Charset::Utf32(endian) => decode_utf32(input, endian),
        Charset::Iso2022Jp => decode_iso2022jp(input),
    }
}

fn single_byte(input: &[u8], map: impl Fn(u8) -> Option<char>) -> (Vec<char>, Option<Error>) {
    let mut chars = Vec::with_capacity(input.len());
    for &b in input {
        match map(b) {
            Some(c) => chars.push(c),
            None => return (chars, Some(Error::Illegal)),
        }
    }
    (chars, None)
}

fn decode_utf8(input: &[u8]) -> (Vec<char>, Option<Error>) {
    match std::str::from_utf8(input) {
        Ok(text) => (text.chars().collect(), None),
        Err(e) => {
            let valid = std::str::from_utf8(&input[..e.valid_up_to()]).unwrap_or_default();
            let error = if e.error_len().is_none() {
                Error::Incomplete
            } else {
                Error::Illegal
            };
            (valid.chars().collect(), Some(error))
        }
    }
}

fn unit16(pair: [u8; 2], endian: Endian) -> u16 {
    match endian {
        Endian::Big => u16::from_be_bytes(pair),
        Endian::Little => u16::from_le_bytes(pair),
    }
}

fn decode_utf16(
    input: &[u8],
    endian: Option<Endian>,
    surrogates: bool,
) -> (Vec<char>, Option<Error>) {
    let mut chars = Vec::new();
    let mut rest = input;
    let endian = match endian {
        Some(e) => e,
        None => match rest {
            [0xFF, 0xFE, ..] => {
                rest = &rest[2..];
                Endian::Little
            }
            [0xFE, 0xFF, ..] => {
                rest = &rest[2..];
                Endian::Big
            }
            _ => Endian::Little,
        },
    };
    while !rest.is_empty() {
        let [a, b, ..] = *rest else {
            return (chars, Some(Error::Incomplete));
        };
        let unit = unit16([a, b], endian);
        rest = &rest[2..];
        let c = match unit {
            0xD800..=0xDBFF if surrogates => {
                let [c, d, ..] = *rest else {
                    return (chars, Some(Error::Incomplete));
                };
                let low = unit16([c, d], endian);
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return (chars, Some(Error::Illegal));
                }
                rest = &rest[2..];
                char::from_u32(
                    0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(low) - 0xDC00),
                )
            }
            _ => char::from_u32(u32::from(unit)),
        };
        match c {
            Some(c) => chars.push(c),
            None => return (chars, Some(Error::Illegal)),
        }
    }
    (chars, None)
}

fn decode_utf32(input: &[u8], endian: Option<Endian>) -> (Vec<char>, Option<Error>) {
    let mut chars = Vec::new();
    let mut rest = input;
    let endian = match endian {
        Some(e) => e,
        None => match rest {
            [0xFF, 0xFE, 0, 0, ..] => {
                rest = &rest[4..];
                Endian::Little
            }
            [0, 0, 0xFE, 0xFF, ..] => {
                rest = &rest[4..];
                Endian::Big
            }
            _ => Endian::Little,
        },
    };
    while !rest.is_empty() {
        let [a, b, c, d, ..] = *rest else {
            return (chars, Some(Error::Incomplete));
        };
        rest = &rest[4..];
        let unit = match endian {
            Endian::Big => u32::from_be_bytes([a, b, c, d]),
            Endian::Little => u32::from_le_bytes([a, b, c, d]),
        };
        match char::from_u32(unit) {
            Some(c) => chars.push(c),
            None => return (chars, Some(Error::Illegal)),
        }
    }
    (chars, None)
}

/// JIS X 0208 pairs where glibc's table (JIS0208.TXT) and WHATWG's index
/// (`encoding_rs`) give different characters: glibc's character.
const GLIBC_JIS0208: [([u8; 2], char); 8] = [
    ([0x21, 0x3D], '\u{2015}'),
    ([0x21, 0x40], '\u{FF3C}'),
    ([0x21, 0x41], '\u{301C}'),
    ([0x21, 0x42], '\u{2016}'),
    ([0x21, 0x5D], '\u{2212}'),
    ([0x21, 0x71], '\u{00A2}'),
    ([0x21, 0x72], '\u{00A3}'),
    ([0x22, 0x4C], '\u{00AC}'),
];

/// A pair in a row of JIS X 0208 itself (rows 1 to 8 and 16 to 84):
/// WHATWG's index also has NEC's and IBM's rows, which glibc does not.
fn jis0208_row(pair: [u8; 2]) -> bool {
    (0x21..=0x7E).contains(&pair[1]) && matches!(pair[0], 0x21..=0x28 | 0x30..=0x74)
}

/// The character of a JIS X 0208 pair, as glibc reads it.
fn jis0208_char(pair: [u8; 2]) -> Option<char> {
    if !jis0208_row(pair) {
        return None;
    }
    if let Some((_, c)) = GLIBC_JIS0208.iter().find(|(p, _)| *p == pair) {
        return Some(*c);
    }
    let bytes = [0x1B, b'$', b'B', pair[0], pair[1]];
    let mut decoder = encoding_rs::ISO_2022_JP.new_decoder_without_bom_handling();
    let mut text = String::with_capacity(16);
    let (result, _) = decoder.decode_to_string_without_replacement(&bytes, &mut text, true);
    let mut chars = text.chars();
    match (result, chars.next(), chars.next()) {
        (encoding_rs::DecoderResult::InputEmpty, Some(c), None) => Some(c),
        _ => None,
    }
}

/// The JIS X 0208 pair of a character, as glibc writes it.
fn jis0208_pair(c: char) -> Option<[u8; 2]> {
    if let Some((pair, _)) = GLIBC_JIS0208.iter().find(|(_, g)| *g == c) {
        return Some(*pair);
    }
    let mut encoder = encoding_rs::ISO_2022_JP.new_encoder();
    let mut buf = [0u8; 16];
    let text = c.to_string();
    let (result, _, written) = encoder.encode_from_utf8_without_replacement(&text, &mut buf, true);
    // ESC $ B, the pair, ESC ( B
    if result != encoding_rs::EncoderResult::InputEmpty || written != 8 || buf[..3] != *b"\x1b$B" {
        return None;
    }
    let pair = [buf[3], buf[4]];
    // only a pair that reads back as this character (WHATWG's encoder
    // also writes look-alikes, and glibc reads some pairs differently)
    (jis0208_char(pair) == Some(c)).then_some(pair)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JisMode {
    Ascii,
    Roman,
    Jis0208,
}

fn decode_iso2022jp(input: &[u8]) -> (Vec<char>, Option<Error>) {
    let mut chars = Vec::new();
    let mut mode = JisMode::Ascii;
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        if b == 0x1B {
            let rest = &input[i + 1..];
            let switch = match rest {
                [b'(', b'B', ..] => Some(JisMode::Ascii),
                [b'(', b'J', ..] => Some(JisMode::Roman),
                [b'$', b'@' | b'B', ..] => Some(JisMode::Jis0208),
                [] | [b'(' | b'$'] => return (chars, Some(Error::Incomplete)),
                _ => None,
            };
            if let Some(next) = switch {
                mode = next;
                i += 3;
                continue;
            }
            // an escape glibc does not know is read as the character ESC
            chars.push('\u{1B}');
            i += 1;
            continue;
        }
        if b >= 0x80 {
            return (chars, Some(Error::Illegal));
        }
        match mode {
            JisMode::Ascii => chars.push(char::from(b)),
            JisMode::Roman => chars.push(match b {
                0x5C => '\u{A5}',
                0x7E => '\u{203E}',
                _ => char::from(b),
            }),
            JisMode::Jis0208 if b < 0x21 => chars.push(char::from(b)),
            JisMode::Jis0208 => {
                let Some(&second) = input.get(i + 1) else {
                    return (chars, Some(Error::Incomplete));
                };
                match jis0208_char([b, second]) {
                    Some(c) => chars.push(c),
                    None => return (chars, Some(Error::Illegal)),
                }
                i += 1;
            }
        }
        i += 1;
    }
    (chars, None)
}

fn encode_iso2022jp(chars: &[char], out: &mut Vec<u8>) -> Result<(), Error> {
    let mut mode = JisMode::Ascii;
    let mut switch = |out: &mut Vec<u8>, to: JisMode| {
        if mode != to {
            out.extend_from_slice(match to {
                JisMode::Ascii => b"\x1b(B",
                JisMode::Roman => b"\x1b(J",
                JisMode::Jis0208 => b"\x1b$B",
            });
            mode = to;
        }
    };
    for &c in chars {
        if c.is_ascii() {
            switch(out, JisMode::Ascii);
            out.push(u8::try_from(u32::from(c)).map_err(|_| Error::Illegal)?);
        } else if c == '\u{A5}' || c == '\u{203E}' {
            switch(out, JisMode::Roman);
            out.push(if c == '\u{A5}' { 0x5C } else { 0x7E });
        } else {
            let pair = jis0208_pair(c).ok_or(Error::Illegal)?;
            switch(out, JisMode::Jis0208);
            out.extend_from_slice(&pair);
        }
    }
    switch(out, JisMode::Ascii);
    Ok(())
}

/// The bytes of `chars` in `charset`.
fn encode(charset: Charset, chars: &[char]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(chars.len());
    match charset {
        Charset::Utf8 => {
            let mut buf = [0; 4];
            for c in chars {
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
        Charset::Ascii => encode_single(chars, &mut out, |c| {
            u8::try_from(u32::from(c)).ok().filter(|b| *b < 0x80)
        })?,
        Charset::Latin1 => encode_single(chars, &mut out, |c| u8::try_from(u32::from(c)).ok())?,
        Charset::Latin9 => encode_single(chars, &mut out, |c| {
            (0..=255).find(|&b| latin9(b) == c)
        })?,
        Charset::Cp1252 => encode_single(chars, &mut out, |c| {
            (0..=255).find(|&b| cp1252(b) == Some(c))
        })?,
        Charset::Utf16(endian) => {
            if endian.is_none() && !chars.is_empty() {
                out.extend_from_slice(&[0xFF, 0xFE]);
            }
            let endian = endian.unwrap_or(Endian::Little);
            let mut buf = [0; 2];
            for c in chars {
                for unit in c.encode_utf16(&mut buf) {
                    push16(&mut out, *unit, endian);
                }
            }
        }
        Charset::Ucs2 => {
            for &c in chars {
                let unit = u16::try_from(u32::from(c)).map_err(|_| Error::Illegal)?;
                push16(&mut out, unit, Endian::Little);
            }
        }
        Charset::Utf32(endian) => {
            if endian.is_none() && !chars.is_empty() {
                out.extend_from_slice(&[0xFF, 0xFE, 0, 0]);
            }
            for &c in chars {
                let bytes = match endian {
                    Some(Endian::Big) => u32::from(c).to_be_bytes(),
                    _ => u32::from(c).to_le_bytes(),
                };
                out.extend_from_slice(&bytes);
            }
        }
        Charset::Iso2022Jp => encode_iso2022jp(chars, &mut out)?,
    }
    Ok(out)
}

fn push16(out: &mut Vec<u8>, unit: u16, endian: Endian) {
    out.extend_from_slice(&match endian {
        Endian::Big => unit.to_be_bytes(),
        Endian::Little => unit.to_le_bytes(),
    });
}

fn encode_single(
    chars: &[char],
    out: &mut Vec<u8>,
    map: impl Fn(char) -> Option<u8>,
) -> Result<(), Error> {
    for &c in chars {
        out.push(map(c).ok_or(Error::Illegal)?);
    }
    Ok(())
}

/// ISO-8859-15: ISO-8859-1 with eight characters changed.
fn latin9(b: u8) -> char {
    match b {
        0xA4 => '€',
        0xA6 => 'Š',
        0xA8 => 'š',
        0xB4 => 'Ž',
        0xB8 => 'ž',
        0xBC => 'Œ',
        0xBD => 'œ',
        0xBE => 'Ÿ',
        _ => char::from(b),
    }
}

/// Windows-1252; 0x81, 0x8D, 0x8F, 0x90 and 0x9D are not characters.
fn cp1252(b: u8) -> Option<char> {
    const HIGH: [Option<char>; 32] = [
        Some('€'),
        None,
        Some('‚'),
        Some('ƒ'),
        Some('„'),
        Some('…'),
        Some('†'),
        Some('‡'),
        Some('ˆ'),
        Some('‰'),
        Some('Š'),
        Some('‹'),
        Some('Œ'),
        None,
        Some('Ž'),
        None,
        None,
        Some('‘'),
        Some('’'),
        Some('“'),
        Some('”'),
        Some('•'),
        Some('–'),
        Some('—'),
        Some('˜'),
        Some('™'),
        Some('š'),
        Some('›'),
        Some('œ'),
        None,
        Some('ž'),
        Some('Ÿ'),
    ];
    match b {
        0x80..=0x9F => HIGH[usize::from(b - 0x80)],
        _ => Some(char::from(b)),
    }
}
