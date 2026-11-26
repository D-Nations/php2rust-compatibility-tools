# php2rust-date

PHP's date extension: `DateTimeImmutable`, `DateTimeZone`, the date text
parser behind the constructor and `modify()`, `format()`,
`createFromFormat()`, `createFromTimestamp()`, the setters and the default
time zone. Written from the PHP manual and from answers recorded from PHP
8.4.27; no PHP source code is used. MIT.

Moved from symfony-rs's symfony-clock crate (`src/php/`), which now
re-exports it as `symfony_clock::php`.

| Contract | Evidence |
|---|---|
| `DateTimeZone::new()`: IANA identifiers (any case, name kept as written), offsets, the zero-offset abbreviations `UTC`, `GMT`, `UCT`, `Z`; `getName()`, `timezone_type` | oracle `timezones` (52, 4 refused) |
| `date_parse()` and `new DateTimeImmutable($text, $zone)`: dates, times, zones, `@timestamp`, relative offsets and words in the recorded texts | oracle `parse` (292 texts x 4 zones) |
| `modify()`, with PHP's gap and fold rules | oracle `modify` (5,840, 1,100 refused) |
| `format()`: all format characters | oracle `format` (44 instants, every character) |
| `createFromFormat()` with `getLastErrors()` warnings and errors | oracle `from_format` (202) |
| `createFromTimestamp()` of ints and floats, `DateRangeError` | oracle `timestamps` (636) |
| `setTime()`, `setDate()`, `setISODate()`, `setTimestamp()`, `setMicrosecond()`, `setTimezone()` | oracle `setters` (264, 5 refused) |
| `date_default_timezone_get()` / `date_default_timezone_set()` (a process-wide `RwLock`, UTC by default) | oracle `default_timezone` |
| Exception classes and messages (`DateMalformedStringException`, `DateInvalidTimeZoneException`, `DateRangeError`, ...) | the oracle answers compare class and message |

Vectors: `scripts/record.sh date` runs `oracle/recorders/php_date.php` and
writes `tests/data/php_date.json`; `tests/php_date.rs` replays it.

## Differences

- Date texts: the parser reads the part of PHP's grammar that the
  recorded cases use. Other texts are refused with
  `DateError::Unsupported`, never answered differently. The test pins the
  number of refused cases in each section.
- A setter that gives a local time in a DST gap is refused
  (`DateError::Unsupported`); PHP keeps the written local time.
- `date_parse()` returns the date and time fields only, not PHP's
  `warnings`, `errors` or relative parts.
- Zone data comes from jiff's bundled tzdb, so answers do not change with
  the host. PHP bundles its own copy; zones whose rules changed between
  the two copies can differ.
- php2rust-value has an older, smaller date module (`datetime`,
  `date_create`, `dateparse`) that Yaml, HttpFoundation and DBAL use.
  The two will merge into this crate.
