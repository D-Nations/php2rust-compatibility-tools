use std::fmt;
use std::rc::Rc;

/// A PHP string: an immutable-by-default byte string with value semantics.
///
/// PHP strings are bytes, not UTF-8, so this wraps `Vec<u8>`. The `Rc` makes
/// copies cheap; [`PhpString::make_mut`] copies on write, which keeps `.=` in
/// a loop linear instead of quadratic.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhpString(Rc<Vec<u8>>);

impl PhpString {
    pub fn new(bytes: Vec<u8>) -> Self {
        PhpString(Rc::new(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Mutable access to the bytes, cloning them first if they are shared.
    pub fn make_mut(&mut self) -> &mut Vec<u8> {
        Rc::make_mut(&mut self.0)
    }

    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.0).into_owned()
    }
}

impl fmt::Debug for PhpString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(&self.0))
    }
}

impl From<Vec<u8>> for PhpString {
    fn from(bytes: Vec<u8>) -> Self {
        PhpString::new(bytes)
    }
}

impl From<&[u8]> for PhpString {
    fn from(bytes: &[u8]) -> Self {
        PhpString::new(bytes.to_vec())
    }
}

impl From<&str> for PhpString {
    fn from(s: &str) -> Self {
        PhpString::new(s.as_bytes().to_vec())
    }
}

impl From<String> for PhpString {
    fn from(s: String) -> Self {
        PhpString::new(s.into_bytes())
    }
}
