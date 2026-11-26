# php2rust-iconv

PHP's `iconv($from, $to, $string)` as PHP 8.4.27 on glibc converts, for a
set of charsets. Written from answers recorded from PHP; no PHP or glibc
source code is used. MIT.

| Contract | Evidence |
|---|---|
| `iconv()` between UTF-8, UTF-16 (`BE`, `LE`), UTF-32 (`BE`, `LE`), UCS-2, ASCII, ISO-8859-1, ISO-8859-15, Windows-1252 and ISO-2022-JP (names in any case, with the usual aliases): the result, or `Error::Illegal`/`Error::Incomplete` where PHP returns `false` (with `Error::notice()`, PHP's `E_NOTICE` text). Byte order marks: written by `UTF-16`/`UTF-32` (little-endian), read if present (else little-endian); `UCS-2` has none. ISO-2022-JP: ASCII, JIS-Roman and JIS X 0208 with glibc's table (no NEC/IBM rows, no half-width katakana), mode switches as glibc writes them | 4,800 recorded calls (`tests/data/iconv.json`, `oracle/recorders/php_iconv.php`, `scripts/record.sh iconv`) |
| Refused (`Error::Unsupported`): any other charset, `//TRANSLIT` and `//IGNORE`, and UTF-8 to UTF-8 of the old forms beyond U+10FFFF (glibc copies them) | Unit test; the recorded calls with other charsets are checked to be refused |

Dependency: `encoding_rs` for the JIS X 0208 index (checked against
glibc's answers pair by pair as it is used).
