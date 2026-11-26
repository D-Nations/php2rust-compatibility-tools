//! `iconv()` against PHP 8.4.27's answers (glibc's iconv) recorded by
//! `oracle/recorders/php_iconv.php` (`scripts/record.sh iconv`): UTF-8 to
//! each charset and back, and arbitrary bytes read as each charset. Calls
//! with a charset this crate refuses must be refused, not answered.
//!
//! Upstream: none
#![allow(clippy::unwrap_used, clippy::expect_used)]

use php2rust_iconv::{Error, iconv};
use serde_json::Value as Json;

fn base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|&c| c != b'=') {
        let v = u32::try_from(ALPHABET.iter().position(|&a| a == c).unwrap()).unwrap();
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(buffer >> bits).unwrap());
            buffer &= (1 << bits) - 1;
        }
    }
    out
}

#[test]
fn iconv_matches_php() {
    let data: Json = serde_json::from_str(include_str!("data/iconv.json")).unwrap();
    let (mut checked, mut refused) = (0, 0);
    let mut failures = Vec::new();
    for case in data["cases"].as_array().unwrap() {
        let (from, to) = (case["from"].as_str().unwrap(), case["to"].as_str().unwrap());
        let input = base64(case["in"].as_str().unwrap());
        let got = iconv(from, to, &input);
        if let Err(Error::Unsupported(_)) = got {
            refused += 1;
            continue;
        }
        checked += 1;
        let out = &case["out"];
        let expected: Result<Vec<u8>, String> = match out[0].as_str().unwrap() {
            "ok" => Ok(base64(out[1].as_str().unwrap())),
            _ => Err(out[1].as_str().unwrap().to_owned()),
        };
        let got = got.map_err(|e| e.notice().unwrap().to_owned());
        if got != expected {
            failures.push(format!(
                "{from} -> {to} {input:02x?}: got {got:02x?}, expected {expected:02x?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(checked > 800, "{checked} checked, {refused} refused");
}

#[test]
fn other_charsets_and_suffixes_are_refused() {
    assert_eq!(
        iconv("UTF-8", "SJIS", b"a"),
        Err(Error::Unsupported("SJIS".to_owned()))
    );
    assert_eq!(
        iconv("UTF-8", "ASCII//TRANSLIT", b"a"),
        Err(Error::Unsupported("ASCII//TRANSLIT".to_owned()))
    );
}
