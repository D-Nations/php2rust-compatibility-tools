use std::collections::{HashMap, HashSet};

use crate::error::{PhpError, PhpResult};
use crate::value::Value;

/// Everything one PHP request owns: its output buffer, user constants, and
/// `include_once` bookkeeping.
///
/// A `Context` is created per request and dropped at the end of it. Nothing
/// outlives the request, which rules out the PHP2Shell bug class (request
/// state left behind for the next request on the same worker) by construction.
#[derive(Default)]
pub struct Context {
    output: Vec<u8>,
    constants: HashMap<Vec<u8>, Value>,
    included: HashSet<&'static str>,
    silence: u32,
    warnings: Vec<String>,
}

impl Context {
    pub fn new() -> Self {
        Context::default()
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }

    /// Records an `E_WARNING`. Warnings go to stderr rather than into the page.
    pub fn warn(&mut self, message: impl Into<String>) {
        self.report("Warning", &message.into());
    }

    pub fn deprecated(&mut self, message: impl Into<String>) {
        self.report("Deprecated", &message.into());
    }

    fn report(&mut self, level: &str, message: &str) {
        if self.silence == 0 {
            eprintln!("PHP {level}:  {message}");
            self.warnings.push(format!("{level}: {message}"));
        }
    }

    /// Warnings and deprecations emitted so far (excluding `@`-silenced ones).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `@expr`: suppresses diagnostics while `f` runs.
    pub fn silenced<T>(&mut self, f: impl FnOnce(&mut Context) -> T) -> T {
        self.silence += 1;
        let result = f(self);
        self.silence -= 1;
        result
    }

    /// `define()` / `const`. Returns false (and warns) if already defined.
    pub fn define(&mut self, name: &[u8], value: Value) -> bool {
        if self.constants.contains_key(name) {
            self.warn(format!(
                "Constant {} already defined",
                String::from_utf8_lossy(name)
            ));
            return false;
        }
        self.constants.insert(name.to_vec(), value);
        true
    }

    pub fn constant(&self, name: &[u8]) -> PhpResult<Value> {
        self.constants.get(name).cloned().ok_or_else(|| {
            PhpError::error(format!(
                "Undefined constant \"{}\"",
                String::from_utf8_lossy(name)
            ))
        })
    }

    /// For `include_once` / `require_once`: true the first time `file` is seen.
    pub fn first_inclusion(&mut self, file: &'static str) -> bool {
        self.included.insert(file)
    }
}
