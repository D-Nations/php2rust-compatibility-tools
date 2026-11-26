# Instructions for agents

These crates copy PHP's runtime behaviour in Rust. Rules:

- No PHP source code in this repository, under any licence (php-src
  included), and no PCRE or libxml source. PHP's behaviour is learned by
  running it (`oracle/`) and from its documentation, then written
  independently.
- Copy behaviour, not style. If PHP's behaviour is surprising, keep it and
  pin it with a test. If it cannot be kept, refuse the input with an error
  rather than approximate it.
- Every new function or rule comes with vectors recorded from PHP 8.4.27
  (a recorder in `oracle/recorders/`, a target in `scripts/record.sh`) and
  a row in the crate README's contract table. Nothing outside the table is
  claimed.
- No `unsafe`. No `unwrap`/`expect` on input; return an error. Prefer fewer
  dependencies.
- Keep `publish = false` until the owner decides to publish.
- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` and `cargo test --workspace` before a push. There is no hosted
  CI; do not add GitHub workflows.
- Small commits; name the PHP function or area in the message.
- Keep the MIT licence and its copyright lines.
