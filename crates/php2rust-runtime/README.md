# php2rust-runtime

The runtime library for Rust code that the php2rust converter generates.
Generated code imports `use php2rust_runtime::prelude::*;` and works on
`Value`s in a per-request `Context`. There is no global mutable state.

| Module | What it copies from PHP |
|---|---|
| `value` | `Value` (a PHP `zval`): null, bool, int, float, string, array, with type juggling |
| `array` | `PhpArray`: ordered keys, PHP's key rules (`Key`), copy on write |
| `string` | `PhpString`: byte strings, not UTF-8 |
| `numeric` | Numeric strings, int/float conversions, `zend_dval_to_lval` |
| `rt` | Operators, including `<=>` and `==` |
| `format` | Number and value output |
| `builtins` | String, array and math functions |

**Status.** Moved unchanged from php2rust (with its git history). Its tests
are written from PHP's documented behaviour; they do not replay vectors
recorded from the oracle yet, so this table lists areas, not a checked
contract. It keeps its own lint set (see `Cargo.toml`). It overlaps with
php2rust-value and will merge with it, one area at a time.
