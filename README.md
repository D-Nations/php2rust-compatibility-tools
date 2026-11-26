# php2rust-compatibility-tools

Rust libraries that copy PHP's runtime behaviour, for projects that port PHP
code to Rust (by hand or with a converter). Each rule is written from PHP
8.4's observed behaviour and its manual, and tests replay answers recorded
from real PHP 8.4.27. No PHP, PCRE or libxml source code is used.

Part of the php2rust project. php2rust-value, php2rust-regex and
php2rust-dom came from [symfony-rs](https://github.com/D-Nations/symfony-rs),
and php2rust-runtime came from the php2rust converter, all with their git
history.

| Crate | What it copies from PHP |
|---|---|
| [php2rust-value](crates/php2rust-value) | Values (`Value`, `PhpArray` with PHP's key rules, objects with identity), `==` and the string/number conversions, and built-in functions: `json_encode`/`json_decode`, `serialize`, `filter_var`, `parse_str`, `parse_url`, `http_build_query`, `html_entity_decode`, base64, `DateTime`/`DateInterval` and date parsing, BcMath |
| [php2rust-regex](crates/php2rust-regex) | `preg_*` patterns (delimiters, modifiers, PCRE2 syntax), translated to `fancy-regex`; what cannot be translated is refused |
| [php2rust-dom](crates/php2rust-dom) | A part of the DOM extension (`DOMDocument::loadXML()`, the read API, `schemaValidateSource()`, a `DOMXPath::evaluate()` subset) with libxml's diagnostics |
| [php2rust-runtime](crates/php2rust-runtime) | The runtime for Rust code that the php2rust converter generates: `Value` with type juggling, copy-on-write `PhpArray`, byte strings, `<=>` and number rules, a per-request `Context`, and string, array and math built-ins |

**Scope.** The crates cover what symfony-rs and php2rust needed so far, not
all of PHP. php2rust-runtime and php2rust-value overlap (both have a PHP
value type); they will merge, one area at a time.
Each crate README has a contract table (php2rust-runtime not yet): only what
it lists is claimed. Add
functions when a project needs them, with vectors recorded from PHP.

**Status.** Not published to crates.io (`publish = false`); the APIs will
change. Use a git dependency pinned to a commit:

```toml
[dependencies]
php2rust-value = { git = "https://github.com/D-Nations/php2rust-compatibility-tools", rev = "<commit>" }
```

To work on this repository and a user of it at the same time, override the
git source in the user's (uncommitted) `.cargo/config.toml`:

```toml
[patch."https://github.com/D-Nations/php2rust-compatibility-tools"]
php2rust-value = { path = "../php2rust-compatibility-tools/crates/php2rust-value" }
```

## Checks

There is no hosted CI. Run the checks locally:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Some functions are also tested through symfony-rs's components (for
example php2rust-regex through symfony-finder's 137 recorded patterns, and
`parse_str` and the IP functions through http-foundation's vectors). Run
symfony-rs's tests after a change here.

## Recording vectors

`scripts/record.sh <target>` runs a recorder in `oracle/recorders/` in the
PHP 8.4.27 Docker image (`oracle/`) and writes the crate's
`tests/data/*.json`. The `dom` targets also need a symfony-rs checkout with
`upstream/` fetched and a symfony/demo checkout (see `oracle/compose.yaml`).

## Licence

MIT, see [LICENSE](LICENSE). Not affiliated with or endorsed by the PHP
Group, Symfony or the Doctrine Project.
