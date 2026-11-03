use std::error::Error;
use std::fmt;

/// Why a PHP script stopped early.
#[derive(Clone, Debug, PartialEq)]
pub enum PhpError {
    /// An uncaught `Throwable`, identified by class name until classes land.
    Thrown {
        /// The class name, e.g. `TypeError`.
        class: &'static str,
        /// The message PHP prints after the class name.
        message: String,
    },
    /// `exit` / `die`. Not an error: it unwinds the script with a status code.
    Exit(i32),
}

/// The result of anything that can throw or exit.
pub type PhpResult<T> = Result<T, PhpError>;

impl PhpError {
    fn thrown(class: &'static str, message: impl Into<String>) -> Self {
        PhpError::Thrown {
            class,
            message: message.into(),
        }
    }

    /// A plain `Error`.
    pub fn error(message: impl Into<String>) -> Self {
        Self::thrown("Error", message)
    }

    /// A `TypeError`.
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::thrown("TypeError", message)
    }

    /// A `ValueError`.
    pub fn value_error(message: impl Into<String>) -> Self {
        Self::thrown("ValueError", message)
    }

    /// An `ArgumentCountError`.
    pub fn argument_count_error(message: impl Into<String>) -> Self {
        Self::thrown("ArgumentCountError", message)
    }

    /// An `ArithmeticError`.
    pub fn arithmetic_error(message: impl Into<String>) -> Self {
        Self::thrown("ArithmeticError", message)
    }

    /// A `DivisionByZeroError`.
    pub fn division_by_zero(message: impl Into<String>) -> Self {
        Self::thrown("DivisionByZeroError", message)
    }

    /// An `UnhandledMatchError`.
    pub fn unhandled_match(message: impl Into<String>) -> Self {
        Self::thrown("UnhandledMatchError", message)
    }
}

impl fmt::Display for PhpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhpError::Thrown { class, message } => write!(f, "{class}: {message}"),
            PhpError::Exit(code) => write!(f, "exit({code})"),
        }
    }
}

impl Error for PhpError {}
