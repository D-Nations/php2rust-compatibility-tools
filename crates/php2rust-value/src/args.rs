//! Binding call arguments to parameters: `$f(...$arguments)` where
//! `$arguments` has int keys (positional) and string keys (named
//! arguments, PHP 8.1), for a function or method without variadic or
//! by-reference parameters. PHP's errors and their messages are kept; the
//! " in FILE on line N" part of "Too few arguments" is left out (no Rust
//! caller has a PHP file and line).
//!
//! Upstream: none

use crate::{
    array::{Key, PhpArray},
    value::{Thrown, Value},
};

/// A declared parameter: its name (without `$`) and its default value,
/// `None` when it is required.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub default: Option<Value>,
}

impl Parameter {
    pub fn required(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            default: None,
        }
    }

    pub fn optional(name: &str, default: Value) -> Self {
        Self {
            name: name.to_owned(),
            default: Some(default),
        }
    }
}

fn error(class: &str, message: String) -> Thrown {
    Thrown::new(class, message.into_bytes(), 0)
}

/// The values `function` (`Class::method` in messages) receives for
/// `...$arguments`: one per parameter, defaults filled in, then the extra
/// positional arguments (`func_get_args()` after the declared ones). An
/// optional parameter before a required one counts as required, as PHP 8
/// treats it (with a deprecation at declaration).
pub fn bind(
    function: &str,
    parameters: &[Parameter],
    arguments: &PhpArray,
) -> Result<Vec<Value>, Thrown> {
    let mut slots: Vec<Option<Value>> = vec![None; parameters.len()];
    let mut extra = Vec::new();
    let mut positional = 0;
    let mut named = false;
    for (key, value) in arguments.iter() {
        match key {
            Key::Int(_) => {
                if named {
                    return Err(error(
                        "Error",
                        "Cannot use positional argument after named argument during unpacking"
                            .to_owned(),
                    ));
                }
                match slots.get_mut(positional) {
                    Some(slot) => *slot = Some(value.clone()),
                    None => extra.push(value.clone()),
                }
                positional += 1;
            }
            Key::Str(name) => {
                named = true;
                let name = String::from_utf8_lossy(name.as_bytes()).into_owned();
                let Some(index) = parameters.iter().position(|p| p.name == name) else {
                    return Err(error("Error", format!("Unknown named parameter ${name}")));
                };
                if slots[index].is_some() {
                    return Err(error(
                        "Error",
                        format!("Named parameter ${name} overwrites previous argument"),
                    ));
                }
                slots[index] = Some(value.clone());
            }
        }
    }
    let required = parameters
        .iter()
        .rposition(|p| p.default.is_none())
        .map_or(0, |i| i + 1);
    let passed = slots.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
    let mut out = Vec::with_capacity(parameters.len() + extra.len());
    for (i, (slot, parameter)) in slots.into_iter().zip(parameters).enumerate() {
        if let Some(value) = slot {
            out.push(value);
        } else if i >= passed && i >= required {
            out.push(parameter.default.clone().unwrap_or(Value::Null));
        } else if i < passed {
            match &parameter.default {
                Some(default) if i >= required => out.push(default.clone()),
                _ => {
                    return Err(error(
                        "ArgumentCountError",
                        format!(
                            "{function}(): Argument #{} (${}) not passed",
                            i + 1,
                            parameter.name
                        ),
                    ));
                }
            }
        } else {
            let bound = if parameters.len() == required {
                "exactly"
            } else {
                "at least"
            };
            return Err(error(
                "ArgumentCountError",
                format!(
                    "Too few arguments to function {function}(), {passed} passed and {bound} {required} expected"
                ),
            ));
        }
    }
    out.extend(extra);
    Ok(out)
}
